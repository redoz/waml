//! The `ClassDiagramSurface` widget: draws the flattened `Scene` under a pan/zoom
//! `Camera`. Read-only — no editing, no hit-testing of individual nodes.
//! Fits the scene to the view on first draw; left-drag pans; scroll zooms
//! toward the cursor. Each node is a filled rect + its title text.
//!
//! Structure/hit-handling mirror the fork's `widgets/src/map/view.rs`.

use super::{
    interaction::ClassInteraction,
    placement::PlacementInteraction,
    selection::{ConstraintVisibility, SelectionPolicy, SelectionState},
    zone_placed, DialPlacement, FrameCommand, InteractionEffects, SceneUpdate, SurfaceIntent,
    TimerCommand, Zone,
};
use crate::canvas::geometry::{
    corner_fillet, elbow_radius, intersect_rect, marker_geometry, segment_quad, snap_bar_to_device,
    ELBOW_MIN_DEVICE_PX,
};
use crate::canvas::viewport::{
    Camera, InitialFit, TimerCommand as ViewportTimerCommand, TouchPair, ViewportController,
    ViewportEffects,
};
use crate::frame::SurfaceExt;
use crate::inspector::Subject;
use crate::popup::base::PopupItem;
use crate::scene::{bounding_box, Scene};
use makepad_widgets::event::{TouchPoint, TouchState, TouchUpdateEvent};
use makepad_widgets::*;
use waml::adornment::{end_marker, End};

script_mod! {
    use mod.prelude.widgets_internal.*
    use mod.atlas
    use mod.widgets.*
    use mod.text.*

    mod.widgets.ClassDiagramSurfaceBase = #(ClassDiagramSurface::register_widget(vm))

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
        // Cross-axis (perpendicular) fade, normalized to the drawn quad. Per axis
        // `1 - clamp((|pos - ctr| - plateau)/soft, 0, 1)`: opaque across the
        // reference span, decaying to 0 at the band edge. Defaults give a constant
        // 1 (plateau 2.0 covers the whole 0..1 quad); the locked axis keeps these
        // defaults, the unlocked axis is overridden per draw (`cross_fade_params`).
        cross_ctr: uniform(vec2(0.5, 0.5))
        cross_plateau: uniform(vec2(2.0, 2.0))
        cross_soft: uniform(vec2(1.0, 1.0))
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
            // Symmetric cross-axis fade, one factor per axis (locked axis ≈ 1).
            let cfx = 1.0 - clamp((abs(self.pos.x - self.cross_ctr.x) - self.cross_plateau.x) / self.cross_soft.x, 0.0, 1.0)
            let cfy = 1.0 - clamp((abs(self.pos.y - self.cross_ctr.y) - self.cross_plateau.y) / self.cross_soft.y, 0.0, 1.0)
            let cross = cfx * cfy
            let a = self.color.w * (0.22 + 0.55 * line) * fade * cross
            sdf.rect(0.0, 0.0, self.rect_size.x, self.rect_size.y)
            sdf.fill(vec4(self.color.x, self.color.y, self.color.z, a))
            return sdf.result
        }
    }

    // Hidden-group border pen (the x-ray, spec §3): a dashed hairline on the
    // group rect with NO fill. The dash rides the (x+y) diagonal so the pattern
    // stays continuous across all four sides and around the corners, unlike
    // per-side stamping. `dash_px` is pushed per draw so the dash grows with
    // zoom. Branch-free: an `if` on a uniform silently no-ops in this fork's
    // shader VM (see EdgeMarker above), so the on/off duty is a 0..1 mask
    // multiplied into the stroke alpha.
    //
    // Task 6 visual review (`tests/fixtures/groups`, x-ray on), RE-RUN after the
    // duty mask below was reworked -- the first review predates that rework and
    // signed off a mask that no longer ships. What ships, confirmed on a native
    // capture of the `Billing` (shrink) group:
    //   * the diagonal-parameterized shader, not the segment-stamping fallback:
    //     the dash runs continuously around all four corners with no bunching
    //     and no phase seam, so the fallback was not needed;
    //   * the symmetric `abs(fract(..) - 0.5)` duty mask: a pixel readout across
    //     the dashed border shows partial-coverage samples on BOTH ends of every
    //     dash (bg 235, ink 182, ends 219/194, 226/187, 185/228), which is the
    //     antialiasing the superseded one-sided `(0.5 - f)` ramp gave only one
    //     end of. `the_dash_mask_filters_both_edges_symmetrically` locks the
    //     idiom in, since the pixel fn cannot run headless.
    mod.draw.GroupDashed = mod.draw.DrawColor{
        dash_px: uniform(6.0)
        stroke_w: uniform(1.0)
        pixel: fn() {
            let p = self.pos * self.rect_size
            let sdf = Sdf2d.viewport(p)
            let inset = self.stroke_w * 0.5
            sdf.rect(inset, inset, self.rect_size.x - inset * 2.0, self.rect_size.y - inset * 2.0)
            // 50% duty cycle with ~1px of antialiasing on BOTH dash edges.
            // `d` is the symmetric triangle-wave distance to the nearest dash
            // centre in PIXELS (the same `abs(fract(..) - 0.5) * period` idiom
            // as ConstraintVeil above); inking where it exceeds a quarter
            // period gives the 50% duty. Because the wave is symmetric, the
            // edge where `fract` wraps 1 -> 0 is filtered exactly like the
            // other one -- a one-sided `(0.5 - f)` ramp leaves that wrap a hard
            // step and the dash ends crawl and shimmer under pan and zoom. The
            // `+ 0.5` centres the ~1px blend band on the dash edge.
            let d = abs(fract((p.x + p.y) / self.dash_px) - 0.5) * self.dash_px
            let mask = clamp(d - self.dash_px * 0.25 + 0.5, 0.0, 1.0)
            sdf.stroke(
                vec4(self.color.x, self.color.y, self.color.z, self.color.w * mask),
                self.stroke_w
            )
            return sdf.result
        }
    }

    mod.widgets.ClassDiagramSurface = set_type_default() do mod.widgets.ClassDiagramSurfaceBase{
        width: Fill
        height: Fill
        draw_bg +: { color: atlas.canvas_ground }
        draw_group +: { color: atlas.group_fill }
        // Hidden-group x-ray outline; dim ink so it reads as secondary chrome.
        draw_group_dashed: mod.draw.GroupDashed{ color: atlas.text_dim }
        // Colour-only holder (never drawn): the dim ink copied onto `draw_text`
        // for a hidden group's title, so no RGBA crosses Rust.
        draw_group_title_dim +: { color: atlas.text_dim }
        // Node card: a near-white glass panel carrying the Atlas
        // "source-bright" frame -- the reusable `AccentFrame` primitive (see
        // `frame.rs`): a thin accent stroke fading along a 150deg diagonal,
        // bright top-left (`frame_hi`) to dim bottom-right (`frame_lo`). Only
        // the fill differs from the frame defaults, so we override just `color`.
        // Depth knobs are the svelte `--node` preset (8px / 22px / .14), so a
        // card sits ON the canvas ground rather than being flush with it.
        draw_node: mod.draw.AccentFrame{ color: atlas.field_bg depth_y: 8.0 depth_blur: 22.0 depth_a: 0.14 }
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
pub struct ClassDiagramSurface {
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
    draw_group_dashed: DrawColor,
    /// Colour-only holder for a hidden group's dim title ink (never drawn).
    #[live]
    draw_group_title_dim: DrawColor,
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
    viewport: ViewportController,
    #[rust]
    interaction: ClassInteraction,
    #[rust]
    placement: PlacementInteraction,
    /// Makepad handle for the controller-owned preview animation.
    #[rust]
    preview_frame: NextFrame,
    /// Animation clock for viewport glides. An *interval*, not a `NextFrame`: a
    /// next-frame chain only re-arms once the paint loop has already painted, so
    /// after an idle click it inherits the loop's wake-up ramp and delivers ~5
    /// frames in the first 250ms. A repeating timer wakes the loop on its own
    /// schedule and paces the glide evenly.
    #[rust]
    cam_timer: Timer,
    /// Makepad handle for the controller-owned dwell timeout.
    #[rust]
    dwell_timer: Timer,
    #[rust]
    selection: SelectionState,
}

/// The relations that should be drawn under a visibility mode + sticky selection
/// (spec §1). `None` ⇒ empty; `Selected` ⇒ relations touching `selected_key` as
/// subject OR reference (empty if nothing selected). Pure, GPU-free (mirrors
/// `node_at` selection logic).
fn relations_for_visibility<'a>(
    relations: &'a [crate::scene::SceneRelation],
    mode: ConstraintVisibility,
    selected_key: Option<&str>,
) -> Vec<&'a crate::scene::SceneRelation> {
    match mode {
        ConstraintVisibility::None => Vec::new(),
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

/// CSS-inspired focus state of a canvas element under constraint visibility.
///
/// Modeled as orthogonal boolean states (like `:focus` / a `:related` custom
/// state) rather than a pushed float, so the render layer reads *why* an element
/// is emphasised, not just a magnitude. `selected` is the picked node itself;
/// `related` is a node that shares a visible constraint with it (a direct
/// neighbour). Everything that is neither is out of focus and renders greyscale.
///
/// Shared vocabulary for nodes now; edges adopt the same struct when their mute
/// lands -- an edge is never `selected`, only `related` (it touches the picked
/// node). `disabled` / `hovered` are the natural next orthogonal states and slot
/// in here without disturbing the colour split. Pure + GPU-free so the split is
/// unit-testable.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
struct FocusState {
    selected: bool,
    related: bool,
}

impl FocusState {
    /// Full colour when in focus (the picked node or a constraint neighbour);
    /// greyscale otherwise. The one rule the render layer keys the `grey` uniform
    /// off of.
    fn coloured(self) -> bool {
        self.selected || self.related
    }
}

/// Focus state of node `key` given the picked node and the set of keys sharing a
/// visible constraint with it (the picked key plus every endpoint of the
/// relations touching it). `related` excludes the picked node so the two states
/// stay mutually exclusive -- the selection is `selected`, its neighbours are
/// `related`, everyone else is neither. Pure so the colour/greyscale split is
/// testable without a GPU.
fn node_focus_state(
    key: &str,
    selected_key: Option<&str>,
    focus_keys: &std::collections::HashSet<String>,
) -> FocusState {
    FocusState {
        selected: selected_key == Some(key),
        related: selected_key != Some(key) && focus_keys.contains(key),
    }
}

/// Rec.601 luminance grey of a colour, alpha preserved -- the Rust-side twin of
/// the AccentFrame shader's `grey` stroke mute, used to desaturate a muted card's
/// chromatic body bits (header wash, accent/amber text, port nubs). Same weights
/// as the shader so ring and body land on the same grey.
fn desaturate(c: Vec4) -> Vec4 {
    let l = c.x * 0.299 + c.y * 0.587 + c.z * 0.114;
    vec4(l, l, l, c.w)
}

/// What chrome a group gets this frame (spec §3). Three outcomes, not two: a
/// group either draws its full chrome, draws only the x-ray outline, or draws
/// nothing at all.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum GroupDraw {
    /// Tinted fill + title — a `frame` group, exactly as the canvas drew every
    /// group before this change.
    Chrome,
    /// Dashed hairline + dim title, no fill — a layout-only group revealed by
    /// the hidden-borders x-ray.
    Dashed,
    /// Nothing. A layout-only group with the x-ray off.
    Skip,
}

/// Per-group render decision. Only `Shape::Frame` opts into chrome
/// (`docs/uaml-spec.md:674-681`); `box`/`shrink` reserve space without drawing,
/// which is what the web renderer already does. The hidden-borders toggle is an
/// x-ray that brings the invisible ones back as dashed outlines. Pure, GPU-free.
fn group_draw_mode(shape: waml::syntax::Shape, show_hidden: bool) -> GroupDraw {
    match (shape, show_hidden) {
        (waml::syntax::Shape::Frame, _) => GroupDraw::Chrome,
        (_, true) => GroupDraw::Dashed,
        (_, false) => GroupDraw::Skip,
    }
}

/// Display name for an unnamed group under the x-ray. `SolvedGroup.title` is
/// `None` for inline groups; `n` is a 1-based counter over the untitled groups
/// in `scene.groups` order, so the labels are stable across redraws of the same
/// scene. Renderer-side only — no model change.
fn untitled_label(n: usize) -> String {
    format!("Untitled {n}")
}

/// The title a group draws, given its authored name and its 1-based untitled
/// ordinal (`None` when the group *is* named). A named group always draws its
/// name; an unnamed one only gets the `Untitled N` fallback under the x-ray, so
/// an unnamed `frame` group keeps drawing no title exactly as it did before this
/// change. Pure so the rule is testable without a GPU.
fn group_label(
    title: &Option<String>,
    untitled_n: Option<usize>,
    mode: GroupDraw,
) -> Option<String> {
    match (title, untitled_n) {
        (Some(t), _) => Some(t.clone()),
        (None, Some(n)) if mode == GroupDraw::Dashed => Some(untitled_label(n)),
        _ => None,
    }
}

/// The whole scene's per-group draw decision + title, in `scene.groups` order
/// (one entry per group, `Skip` included, so callers can zip it back onto the
/// groups). The untitled counter advances for *every* unnamed group, drawn or
/// not, so `Untitled N` labels stay stable no matter which groups the x-ray is
/// showing. Pure: this is the whole of `draw_walk`'s group bookkeeping, lifted
/// out so it can be tested.
fn group_plan(
    groups: &[waml::solve::SolvedGroup],
    show_hidden: bool,
) -> Vec<(GroupDraw, Option<String>)> {
    let mut untitled_seen = 0usize;
    groups
        .iter()
        .map(|g| {
            let mode = group_draw_mode(g.shape, show_hidden);
            let untitled = if g.title.is_none() {
                untitled_seen += 1;
                Some(untitled_seen)
            } else {
                None
            };
            (mode, group_label(&g.title, untitled, mode))
        })
        .collect()
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

const FONT_RASTER_SIZES: &[f32] = &[
    32.0, 40.0, 50.0, 63.0, 79.0, 99.0, 124.0, 155.0, 194.0, 243.0, 304.0,
];
/// How far (screen px) a veil hatch reaches from its anchor edge before fully
/// fading. Keeps a half-plane veil from flooding the canvas (spec §2).
const VEIL_REACH: f64 = 420.0;

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

/// Screen-space fill rect for a veil: the keep-out region anchored to the
/// reference's screen rect (spec §2 mapping). Each LOCKED (extend) axis is
/// clamped to `reach` px off the reference edge; each UNLOCKED (cross) axis is
/// bounded to the reference's own span grown by `reach` on each side, centred on
/// the reference (the extent the cross-axis fade decays across -- see
/// `cross_fade_params`), so the veil reads as a soft blob hugging the reference
/// rather than an infinite strip. The caller clips the band to the viewport.
/// Cardinal ⇒ one locked axis; diagonal ⇒ both. Pure, GPU-free (unit-testable
/// like `segment_quad`).
fn veil_band(reference: Rect, dir: waml::syntax::Direction, reach: f64) -> Rect {
    use waml::syntax::Direction::*;
    let (x0, xw) = match dir {
        LeftOf | AboveLeft | BelowLeft => (reference.pos.x, reach),
        RightOf | AboveRight | BelowRight => (reference.pos.x + reference.size.x - reach, reach),
        Above | Below => (reference.pos.x - reach, reference.size.x + 2.0 * reach),
    };
    let (y0, yh) = match dir {
        Above | AboveLeft | AboveRight => (reference.pos.y, reach),
        Below | BelowLeft | BelowRight => (reference.pos.y + reference.size.y - reach, reach),
        LeftOf | RightOf => (reference.pos.y - reach, reference.size.y + 2.0 * reach),
    };
    Rect {
        pos: dvec2(x0, y0),
        size: dvec2(xw, yh),
    }
}

/// Per-axis cross-fade uniforms for `ConstraintVeil`, expressed in the drawn
/// (already view-clipped) `band`'s normalized 0..1 frame: `(ctr, plateau, soft)`
/// vec2 triples. Each axis fades as `1 - clamp((|pos - ctr| - plateau)/soft, 0,
/// 1)`: fully opaque (`1`) across the reference's span and decaying to `0` at the
/// band edge. The LOCKED (extend) axis and both axes of a diagonal get a plateau
/// wider than the quad, so their cross-fade is a constant `1` and only the
/// extend `ramp`/`bias` fade shapes them. Pure, GPU-free.
fn cross_fade_params(
    band: Rect,
    reference: Rect,
    dir: waml::syntax::Direction,
    reach: f64,
) -> ([f32; 2], [f32; 2], [f32; 2]) {
    use waml::syntax::Direction::*;
    // (ctr, plateau, soft) that yields a constant cross-fade of 1: plateau 2.0
    // covers the whole 0..1 quad, so `|pos - ctr| - plateau` is always <= 0.
    let flat = (0.5f32, 2.0f32, 1.0f32);
    // Reference-centred plateau with a `reach`-px soft tail, normalized to `band`.
    let axis = |origin: f64, span: f64, ref_ctr: f64, ref_half: f64| -> (f32, f32, f32) {
        if span <= 0.0 {
            return flat;
        }
        let ctr = ((ref_ctr - origin) / span) as f32;
        let plateau = (ref_half / span) as f32;
        let soft = (reach / span).max(1e-4) as f32;
        (ctr, plateau, soft)
    };
    let (cx, cy) = match dir {
        // x unlocked: fade across the width, centred on the reference's x span.
        Above | Below => (
            axis(
                band.pos.x,
                band.size.x,
                reference.pos.x + reference.size.x * 0.5,
                reference.size.x * 0.5,
            ),
            flat,
        ),
        // y unlocked: fade across the height, centred on the reference's y span.
        LeftOf | RightOf => (
            flat,
            axis(
                band.pos.y,
                band.size.y,
                reference.pos.y + reference.size.y * 0.5,
                reference.size.y * 0.5,
            ),
        ),
        // Diagonals lock both axes; the extend fade already bounds them.
        _ => (flat, flat),
    };
    ([cx.0, cy.0], [cx.1, cy.1], [cx.2, cy.2])
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

/// Overlap length (in the bar's own units) of each corner stub back onto its
/// straight bar. The stub is drawn UN-snapped inside the combined pen but shares
/// the straight bar's snapped centerline + thickness, so its coverage coincides
/// with the straight bar over this overlap -- the butt reads as one continuous
/// bar, no lateral jog. One thickness is plenty to seat the join.

/// How far (as a fraction of the arc-band half-width) each stub reaches PAST its
/// tangent into the arc band, so the stub interpenetrates the band instead of
/// butting it. Without this overlap the stub and the band's flat cap share a
/// zero-crossing exactly on the tangent and `fill` antialiases it to a hairline
/// seam; half the half-width buries the crossing while the straight stub's bulge
/// past the arc's outer curve stays well under a pixel.

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

/// A resolved terminal glyph ready to draw: the axis-aligned quad to place it
/// in, the four packed path vertices in that quad's local pixel space, and the
/// branchless `hollow`/`filled` interior flags the `EdgeMarker` shader reads.

/// Turn a [`Marker`] at an endpoint into drawable geometry, oriented so the glyph
/// points along `dir_raw` (the terminal segment direction, toward the node). The
/// tip sits ON `ep` (the routed endpoint, which lands on the node border); the
/// body extends back along `-dir`. Vertices are emitted in the returned quad's
/// local pixel space to match the shader's `self.pos * self.rect_size` frame.
/// Returns `None` for `Marker::None` or a degenerate (zero-length) direction.
/// Pure, for a GPU-free test.

/// Screen position of a routed world point under `camera`, offset into the
/// canvas `rect`. Mirrors the edge segment loop's world->local->rect math.
fn edge_point_to_screen(camera: &Camera, rect_pos: DVec2, p: (f64, f64)) -> DVec2 {
    let (lx, ly) = camera.world_to_local(p.0, p.1);
    dvec2(rect_pos.x + lx, rect_pos.y + ly)
}

/// Canvas -> App action (same convention as `ToolDockAction`).
#[derive(Clone, Debug, Default, PartialEq)]
pub enum ClassDiagramSurfaceAction {
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
    /// A node-drag armed the compass on a (new) target: the view computes the
    /// per-zone conflict verdicts (speculative solve), pushes them back via
    /// `set_conflict_zones`, and asks the shell to pop the drop dial at
    /// `center`. `subject` = dragged node, `reference` = target.
    CompassArmed {
        subject_key: String,
        reference_key: String,
        center: DVec2,
    },
    /// The drag pulled the cursor out of the open dial's reach: dismiss it so
    /// the drag is free to dwell on another target. (The canvas can't close a
    /// popup itself -- `PopupRoot` is the dismiss authority.)
    DialDismiss,
}

impl From<SurfaceIntent> for ClassDiagramSurfaceAction {
    fn from(intent: SurfaceIntent) -> Self {
        match intent {
            SurfaceIntent::NodeMenu { abs, key } => Self::NodeMenu { abs, key },
            SurfaceIntent::NodeSelect { key } => Self::NodeSelect { key },
            SurfaceIntent::NodeDeselect => Self::NodeDeselect,
            SurfaceIntent::ToggleExpand { key } => Self::ToggleExpand { key },
            SurfaceIntent::CompassArmed {
                subject_key,
                reference_key,
                center,
            } => Self::CompassArmed {
                subject_key,
                reference_key,
                center,
            },
            SurfaceIntent::DialDismiss => Self::DialDismiss,
        }
    }
}

impl Widget for ClassDiagramSurface {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, _scope: &mut Scope) {
        if let Some(te) = self.cam_timer.is_event(event) {
            let effects = self.viewport.tick_camera(te.time.unwrap_or(0.0));
            self.apply_viewport_effects(cx, effects);
        }
        if let Event::KeyDown(ke) = event {
            if ke.key_code == KeyCode::Escape && self.placement.snapshot().dragged_key.is_some() {
                let effects = self.placement.cancel(&mut self.scene, &mut self.viewport);
                self.viewport.suppress_release();
                self.apply_interaction_effects(cx, effects);
                return;
            }
        }
        if self.dwell_timer.is_event(event).is_some() {
            let center = self.placement.cursor_abs;
            let effects = self.placement.dwell_elapsed(&self.scene, center);
            self.apply_interaction_effects(cx, effects);
            return;
        }
        if let Some(ne) = self.preview_frame.is_event(event) {
            let effects = self
                .placement
                .tick_preview(ne.time, &mut self.scene, &mut self.viewport);
            self.apply_interaction_effects(cx, effects);
        }
        if let Event::TouchUpdate(tu) = event {
            if self.handle_pinch(cx, tu) {
                return;
            }
        }
        match event.hits_with_capture_overload(cx, self.draw_bg.area(), false) {
            Hit::FingerDown(fe) if fe.mouse_button() == Some(MouseButton::SECONDARY) => {
                let effects =
                    self.interaction
                        .secondary_down(fe.abs, &self.scene, self.viewport.snapshot());
                self.apply_interaction_effects(cx, effects);
            }
            Hit::FingerDown(fe) if fe.is_primary_hit() => {
                let effects = self.viewport.cancel_glide();
                self.apply_viewport_effects(cx, effects);
                let effects = self.interaction.primary_down(
                    fe.abs,
                    &self.scene,
                    &mut self.viewport,
                    &mut self.placement,
                );
                self.apply_interaction_effects(cx, effects);
                cx.set_cursor(MouseCursor::Grabbing);
            }
            Hit::FingerMove(fe) => {
                let effects = self.interaction.pointer_move(
                    fe.abs,
                    &mut self.scene,
                    &mut self.viewport,
                    &mut self.selection,
                    &mut self.placement,
                );
                self.apply_interaction_effects(cx, effects);
            }
            Hit::FingerUp(fe) if fe.is_primary_hit() => {
                let effects = self.interaction.pointer_up(
                    fe.abs,
                    true,
                    &mut self.scene,
                    &mut self.viewport,
                    &mut self.selection,
                    &mut self.placement,
                );
                self.apply_interaction_effects(cx, effects);
                cx.set_cursor(MouseCursor::Grab);
            }
            Hit::FingerUp(fe) => {
                let effects = self.interaction.pointer_up(
                    fe.abs,
                    false,
                    &mut self.scene,
                    &mut self.viewport,
                    &mut self.selection,
                    &mut self.placement,
                );
                self.apply_interaction_effects(cx, effects);
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
                                                       // Scroll-zoom is its own continuous motion; a button glide still
                                                       // in the air would fight it for the camera.
                let effects = self.viewport.apply_scroll_zoom(fs.abs, factor);
                self.apply_viewport_effects(cx, effects);
            }
            _ => {}
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, _scope: &mut Scope, walk: Walk) -> DrawStep {
        let rect = cx.walk_turtle(walk);
        self.viewport.set_view_rect(rect);
        self.draw_bg.draw_abs(cx, rect);

        self.viewport.apply_initial_fit();
        let selection = self.selection.snapshot();

        // Contents (text offsets, font sizes, hairline weights) scale by the same
        // factor as the box geometry, so a zoomed shape magnifies its interior too.
        let zoom = self.viewport.camera().zoom;
        // Node frame inset + stroke live in draw_node's SDF shader; feed zoom in
        // as a uniform so the border thickens with the box rather than staying a
        // fixed screen-pixel hairline.
        self.draw_node
            .set_uniform(cx, live_id!(zoom), &[zoom as f32]);

        // Groups: only a `frame`-shaped group draws chrome -- `box`/`shrink` are
        // layout-only per docs/uaml-spec.md:674-681, which is what the web
        // renderer already does; this brings native into line. The view bar's
        // hidden-borders x-ray brings the invisible ones back as a dashed
        // hairline with a dim title (`Untitled N` when the group is unnamed).
        // Nesting is unchanged: draw-order (shallow first) leaves inner groups on
        // top. Collect screen rects first so the pens (&mut self) can draw
        // without holding the `self.scene.groups` borrow.
        // Mode + label per group come from the pure `group_plan` seam; only the
        // camera projection stays here.
        let plan = group_plan(&self.scene.groups, selection.show_hidden_borders);
        let group_draws: Vec<(Rect, Option<String>, GroupDraw)> = self
            .scene
            .groups
            .iter()
            .zip(plan)
            .filter_map(|(g, (mode, label))| {
                if mode == GroupDraw::Skip {
                    return None;
                }
                let camera = self.viewport.camera();
                let (lx, ly) = camera.world_to_local(g.rect.x, g.rect.y);
                let screen = Rect {
                    pos: dvec2(rect.pos.x + lx, rect.pos.y + ly),
                    size: dvec2(g.rect.w * camera.zoom, g.rect.h * camera.zoom),
                };
                Some((screen, label, mode))
            })
            .collect();
        // Group titles borrow the shared body pen; stash its ink so the node
        // titles drawn later in this pass are unaffected by the dim override.
        let title_ink = self.draw_text.color;
        let dim_ink = self.draw_group_title_dim.color;
        // Dash period grows with zoom but stays legible at either extreme.
        let dash_px = (6.0 * zoom).clamp(3.0, 18.0) as f32;
        for (screen, label, mode) in group_draws {
            match mode {
                GroupDraw::Chrome => self.draw_group.draw_abs(cx, screen),
                GroupDraw::Dashed => {
                    self.draw_group_dashed
                        .set_uniform(cx, live_id!(dash_px), &[dash_px]);
                    self.draw_group_dashed.draw_abs(cx, screen);
                }
                GroupDraw::Skip => {}
            }
            if let Some(label) = &label {
                self.draw_text.color = if mode == GroupDraw::Dashed {
                    dim_ink
                } else {
                    title_ink
                };
                let size = (12.0 * zoom) as f32;
                let font_size = font_raster_size(size);
                self.draw_text.text_style.font_size = font_size;
                self.draw_text.font_scale = size / font_size;
                self.draw_text.draw_abs(
                    cx,
                    dvec2(screen.pos.x + 6.0 * zoom, screen.pos.y + 4.0 * zoom),
                    label,
                );
            }
        }
        self.draw_text.color = title_ink;

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
                    let (x, y) = self.viewport.camera().world_to_local(p.0, p.1);
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
                    self.draw_elbow.set_uniform(cx, live_id!(bar_in), &f.bar_in);
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
                let camera = self.viewport.camera();
                let ep_to = edge_point_to_screen(&camera, rect.pos, pts[pts.len() - 1]);
                let prev = edge_point_to_screen(&camera, rect.pos, pts[pts.len() - 2]);
                let ep_from = edge_point_to_screen(&camera, rect.pos, pts[0]);
                let next = edge_point_to_screen(&camera, rect.pos, pts[1]);
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
        // Constraint-focus set (greyscale mute): under the Selected POV a picked
        // node with visible constraints keeps itself + its direct constraint
        // neighbours in full colour and mutes every other card to greyscale. The
        // set is every endpoint of the relations touching the selection (which
        // includes the selection); empty when nothing is selected or the
        // selection has no constraints, in which case `focus_active` is false and
        // no card greys. Owned keys so it outlives the `&mut self` draw loop.
        let focus_keys: std::collections::HashSet<String> = relations_for_visibility(
            &self.scene.relations,
            selection.constraint_visibility,
            selection.selected_key.as_deref(),
        )
        .iter()
        .flat_map(|r| [r.subject.clone(), r.reference.clone()])
        .collect();
        let focus_active = !focus_keys.is_empty();
        let selected_key = selection.selected_key.clone();
        for (i, node) in nodes.iter().enumerate() {
            let camera = self.viewport.camera();
            let (lx, ly) = camera.world_to_local(node.rect.x, node.rect.y);
            let screen = Rect {
                pos: dvec2(rect.pos.x + lx, rect.pos.y + ly),
                size: dvec2(node.rect.w * camera.zoom, node.rect.h * camera.zoom),
            };
            // Push the per-node `selected` uniform (1.0 for the picked node,
            // 0.0 otherwise) so its frame widens; every other node draws exactly
            // as before. Same set_uniform-before-draw_abs cadence as `zoom`.
            let selected = if selection.selected_index == Some(i) {
                1.0f32
            } else {
                0.0
            };
            self.draw_node
                .set_uniform(cx, live_id!(selected), &[selected]);
            // Push the per-node `grey` uniform: mute a card that is out of focus
            // (neither the picked node nor a constraint neighbour) to greyscale.
            // Only when a focus is active, so a bare diagram stays full colour.
            let fs = node_focus_state(&node.key, selected_key.as_deref(), &focus_keys);
            let muted = focus_active && !fs.coloured();
            self.draw_node
                .set_uniform(cx, live_id!(grey), &[if muted { 1.0f32 } else { 0.0 }]);
            // Node card: rounded near-white glass fill + source-bright accent
            // frame, both in draw_node's SDF shader (see script_mod above).
            // `draw_surface_abs` pads the quad so the depth shadow falls outside the
            // card instead of clipping at its border (`frame.rs`).
            self.draw_node.draw_surface_abs(cx, screen);

            // Every node renders the full card on top of its frame.
            self.draw_card(cx, screen, node, zoom, muted);
        }

        // Persistent relation overlay: the full projected relation set, always-on
        // at a calm weight, so authored placement is visible at rest. Drawn under
        // the armed-drag overlay's scoped emphasis.
        self.draw_relations_overlay(cx);

        // Conflict focus (spec §4): fade every card except the focused
        // relation's two nodes, so the contradiction is locatable off the
        // error list. Keyed by node key (not conflict index) so it survives a
        // delete-and-refresh of the open list.
        if let Some(keep) = selection.conflict_focus_keys {
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
        if self.placement.snapshot().drag_moved {
            self.draw_drag_overlay(cx, rect);
        }

        DrawStep::done()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CameraPolicy {
    Refit,
    Focus,
    Retain,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ReconciliationPolicy {
    clear_placement: bool,
    selection: SelectionPolicy,
    camera: CameraPolicy,
}

fn reconciliation_policy(update: &SceneUpdate) -> ReconciliationPolicy {
    match update {
        SceneUpdate::Replace => ReconciliationPolicy {
            clear_placement: true,
            selection: SelectionPolicy::Clear,
            camera: CameraPolicy::Refit,
        },
        SceneUpdate::Focus { .. } => ReconciliationPolicy {
            clear_placement: true,
            selection: SelectionPolicy::Clear,
            camera: CameraPolicy::Focus,
        },
        SceneUpdate::PreserveViewport => ReconciliationPolicy {
            clear_placement: true,
            selection: SelectionPolicy::Preserve,
            camera: CameraPolicy::Retain,
        },
    }
}

impl ClassDiagramSurface {
    fn reset_placement_for_scene_change(&mut self, cx: &mut Cx) {
        let effects = self
            .placement
            .cancel_for_scene_change(&mut self.scene, &mut self.viewport);
        self.viewport.suppress_release();
        self.apply_interaction_effects(cx, effects);
    }

    fn reconcile_scene(&mut self, cx: &mut Cx, scene: Scene, update: SceneUpdate) {
        let policy = reconciliation_policy(&update);
        if policy.clear_placement {
            self.reset_placement_for_scene_change(cx);
        }

        let cancel_effects = self.viewport.cancel_glide();
        self.apply_viewport_effects(cx, cancel_effects);
        let bounds = bounding_box(&scene);
        self.selection.reconcile(&scene.nodes, policy.selection);

        match (&update, policy.camera) {
            (SceneUpdate::Replace, CameraPolicy::Refit) => {
                self.viewport.request_initial_fit(match bounds {
                    Some(bounds) => InitialFit::Scene(bounds),
                    None => InitialFit::ScenePending,
                });
            }
            (SceneUpdate::Focus { key }, CameraPolicy::Focus) => {
                let focus = scene
                    .nodes
                    .iter()
                    .find(|node| &node.key == key)
                    .map(|node| InitialFit::Focus(node.rect))
                    .unwrap_or(InitialFit::FocusPending);
                self.viewport.request_initial_fit(focus);
            }
            (SceneUpdate::PreserveViewport, CameraPolicy::Retain) => {
                let effects = self.viewport.retain_for_scene_update(bounds);
                self.apply_viewport_effects(cx, effects);
            }
            _ => unreachable!("reconciliation policy must match its scene update"),
        }

        self.scene = scene;
        self.draw_bg.redraw(cx);
    }

    /// On-screen rect of scene node `i` under the current camera. Mirrors the
    /// draw-time transform in `draw_walk` / `node_at`.
    fn node_screen_rect(&self, i: usize) -> Rect {
        // While a card is in flight it lives at the placement controller's transient ghost
        // position, not its committed scene rect (which stays at the origin
        // slot until drop). Anchor overlays -- the keep-out veil above all --
        // to the ghost so the hatching tracks the card live instead of
        // snapping only when the drag lands.
        let placement = self.placement.snapshot();
        let r = if placement.dragged_key.as_deref() == Some(self.scene.nodes[i].key.as_str()) {
            placement.ghost.unwrap_or(self.scene.nodes[i].rect)
        } else {
            self.scene.nodes[i].rect
        };
        let viewport = self.viewport.snapshot();
        let (lx, ly) = viewport.camera.world_to_local(r.x, r.y);
        Rect {
            pos: dvec2(viewport.view_rect.pos.x + lx, viewport.view_rect.pos.y + ly),
            size: dvec2(r.w * viewport.camera.zoom, r.h * viewport.camera.zoom),
        }
    }

    /// Draw one placement relation's veil: a hatched grey keep-out anchored to the
    /// reference node's near edge, distance-faded on both axes. No connector line.
    /// Card muting is no longer this pen's job -- the per-node `grey` uniform
    /// (`FocusState`) desaturates every out-of-focus card once, over the whole
    /// scene, instead of a per-relation scrim over the keep-out.
    fn draw_veil_for(
        &mut self,
        cx: &mut Cx2d,
        reference_idx: usize,
        dir: waml::syntax::Direction,
        active: bool,
    ) {
        let reference_screen = self.node_screen_rect(reference_idx);
        let band = veil_band(reference_screen, dir, VEIL_REACH);
        // Clip the band to the view so we don't overdraw the whole window.
        let band = intersect_rect(band, self.viewport.snapshot().view_rect);
        if band.size.x <= 0.5 || band.size.y <= 0.5 {
            return;
        }
        let (ramp, bias) = veil_ramp(dir);
        self.draw_veil.set_uniform(cx, live_id!(ramp), &ramp);
        self.draw_veil.set_uniform(cx, live_id!(bias), &bias);
        // Cross-axis fade, normalized to the clipped band (so it survives the
        // view clip above): opaque across the reference span, decaying to the
        // band edge on the unlocked axis; a constant 1 on the locked axis.
        let (cross_ctr, cross_plateau, cross_soft) =
            cross_fade_params(band, reference_screen, dir, VEIL_REACH);
        self.draw_veil
            .set_uniform(cx, live_id!(cross_ctr), &cross_ctr);
        self.draw_veil
            .set_uniform(cx, live_id!(cross_plateau), &cross_plateau);
        self.draw_veil
            .set_uniform(cx, live_id!(cross_soft), &cross_soft);
        // The veil you are actively authoring (the candidate under the dial)
        // reads in the selection accent with a tighter, bolder hatch, so it is
        // legible against the calm neutral grey of the committed keep-outs you
        // are not manipulating.
        if active {
            self.draw_veil.set_uniform(cx, live_id!(hatch_px), &[6.5]);
            self.draw_veil.color = vec4(0.16, 0.52, 0.86, 1.0);
        } else {
            self.draw_veil.set_uniform(cx, live_id!(hatch_px), &[9.0]);
            self.draw_veil.color = vec4(0.42, 0.47, 0.54, 1.0);
        }
        self.draw_veil.draw_abs(cx, band);
    }

    /// Persistent constraint overlay, gated by the visibility mode + sticky
    /// selection (spec §1): None draws nothing, Selected draws only relations
    /// touching the selected node.
    fn draw_relations_overlay(&mut self, cx: &mut Cx2d) {
        let selection = self.selection.snapshot();
        let selected_key = selection.selected_key;
        // Selected mode is the only drawing mode, so the veil always reframes
        // onto the selected node's POV.
        let pov = selected_key.as_deref();
        let mut chosen: Vec<(usize, waml::syntax::Direction, bool)> = relations_for_visibility(
            &self.scene.relations,
            selection.constraint_visibility,
            selected_key.as_deref(),
        )
        .into_iter()
        .filter_map(|rel| {
            let (subject, reference, dir) =
                reframe_to_selected(&rel.subject, &rel.reference, rel.dir, pov);
            // The subject must still exist in the scene for the relation to be
            // real, but the veil is anchored to the reference only.
            self.scene.nodes.iter().position(|n| n.key == subject)?;
            let ri = self.scene.nodes.iter().position(|n| n.key == reference)?;
            // Committed keep-out: static (not being manipulated).
            Some((ri, dir, false))
        })
        .collect();

        // While a hover preview is up the layout is tweened into the candidate
        // the dial wedge would author, but that placement is not in
        // `scene.relations` yet -- so its keep-out would be missing until the
        // drop commits. Draw the candidate veil now, at the previewed positions
        // (`apply_preview` already tweened the node rects), so the hatch reads
        // exactly as the accepted layout would. The candidate is `A <dir> B`
        // with A the dragged selection, so it anchors to the reference B just as
        // the committed relation would (`reframe_to_selected` is a no-op here).
        let placement = self.placement.snapshot();
        if let (Some(zone), Some(reference_key), Some(_)) = (
            placement.compass_zone,
            placement.armed_target_key.as_deref(),
            placement.preview_ghost.as_ref(),
        ) {
            if let (Some(dir), Some(reference_index)) = (
                zone_placed(zone).dir,
                self.scene
                    .nodes
                    .iter()
                    .position(|node| node.key == reference_key),
            ) {
                // The placement being authored: active (accent hatch).
                chosen.push((reference_index, dir, true));
            }
        }

        for (ri, dir, active) in chosen {
            self.draw_veil_for(cx, ri, dir, active);
        }
    }

    /// SPIKE (drag-place, throwaway): draw the live placement overlay -- the
    /// grey origin slot the node left behind, the dock compass over the target
    /// node (eight zones, the hovered one lit), the dragged ghost, and a DSL
    /// readout. All screen-space.
    fn draw_drag_overlay(&mut self, cx: &mut Cx2d, view: Rect) {
        let placement = self.placement.snapshot();
        let (Some(dragged_key), Some(ghost)) = (placement.dragged_key.as_deref(), placement.ghost)
        else {
            return;
        };
        let Some(ni) = self
            .scene
            .nodes
            .iter()
            .position(|node| node.key == dragged_key)
        else {
            return;
        };
        let a_key = self.scene.nodes[ni].key.clone();
        let place = placement.placed;
        let (vx, vy) = (view.pos.x, view.pos.y);

        let to_screen = |r: waml::solve::Rect| -> Rect {
            let camera = self.viewport.camera();
            let (lx, ly) = camera.world_to_local(r.x, r.y);
            Rect {
                pos: dvec2(view.pos.x + lx, view.pos.y + ly),
                size: dvec2(r.w * camera.zoom, r.h * camera.zoom),
            }
        };
        let gs = to_screen(ghost);
        let os = to_screen(self.scene.nodes[ni].rect); // origin (source) slot

        // Origin marker: grey-wash the source slot + outline so it reads as
        // "left behind" -- you can see which node is in flight. Suppressed under
        // a preview, where A hasn't left anything behind: it IS the fixed point.
        if placement.preview_ghost.is_none() {
            let grey_wash = vec4(0.52, 0.57, 0.64, 0.40);
            self.fill_rect(cx, os.pos.x, os.pos.y, os.size.x, os.size.y, grey_wash);
            let grey = vec4(0.62, 0.67, 0.74, 0.85);
            let gt = 1.5;
            self.fill_rect(cx, os.pos.x, os.pos.y, os.size.x, gt, grey);
            self.fill_rect(cx, os.pos.x, os.pos.y + os.size.y - gt, os.size.x, gt, grey);
            self.fill_rect(cx, os.pos.x, os.pos.y, gt, os.size.y, grey);
            self.fill_rect(cx, os.pos.x + os.size.x - gt, os.pos.y, gt, os.size.y, grey);
        }

        // (The dial itself is the shared `RadialPopup`, drawn by `PopupRoot` in
        // the overlay above this canvas -- not here.)

        // Ghost: translucent accent rect tracking the cursor, carrying the
        // dragged node's identity so you can tell *what* is in flight. Under a
        // preview the real node is already drawn there (the camera pins it to
        // the cursor), so ring it instead of stacking a second copy on top.
        if placement.preview_ghost.is_some() {
            let acc = vec4(0.37, 0.63, 1.0, 0.9);
            let t = 2.0;
            self.fill_rect(cx, gs.pos.x, gs.pos.y, gs.size.x, t, acc);
            self.fill_rect(cx, gs.pos.x, gs.pos.y + gs.size.y - t, gs.size.x, t, acc);
            self.fill_rect(cx, gs.pos.x, gs.pos.y, t, gs.size.y, acc);
            self.fill_rect(cx, gs.pos.x + gs.size.x - t, gs.pos.y, t, gs.size.y, acc);
        } else {
            self.fill_rect(
                cx,
                gs.pos.x,
                gs.pos.y,
                gs.size.x,
                gs.size.y,
                vec4(0.37, 0.63, 1.0, 0.22),
            );
        }
        self.draw_mono_bold.text_style.font_size = 12.0;
        self.draw_mono_bold.font_scale = 1.0;
        self.draw_mono_bold
            .draw_abs(cx, dvec2(gs.pos.x + 6.0, gs.pos.y + 6.0), &a_key);

        // DSL readout, top-left of the view: the statement(s) the current zone
        // would author. Empty when no zone is hovered (drop = cancel).
        if let Some(b_key) = placement.armed_target_key {
            self.draw_mono_dim.text_style.font_size = 12.0;
            self.draw_mono_dim.font_scale = 1.0;
            if let Some(d) = place.dir {
                let line = format!("{a_key} {} {b_key}", dir_word(d));
                self.draw_mono_dim
                    .draw_abs(cx, dvec2(vx + 12.0, vy + 10.0), &line);
            }
        }
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
        grey: bool,
    ) {
        use crate::card::{self, Token, Weight};
        use crate::scene::HeaderStyle;
        let placed = card::measure(&card::class_shape(node, &card::mono_sheet()));
        // Accent/dim are read off the mono pens (both already resolved to the live
        // theme) so the wash/dividers/nubs track the card's own palette.
        // `grey` (out-of-focus card under the Selected POV) mutes the chromatic
        // body bits -- header wash, accent/amber text, port nubs -- to their own
        // luminance, the Rust twin of AccentFrame's `grey` stroke mute, so the
        // whole card desaturates coherently, not just its frame ring. `accent_full`
        // / `amber_full` are the un-muted theme colours captured before any pen
        // mutation, so the pens restore cleanly for the next (coloured) card.
        let accent_full = self.draw_mono_accent.color;
        let amber_full = self.draw_mono_amber.color;
        let dim = self.draw_mono_dim.color;
        let accent = if grey {
            desaturate(accent_full)
        } else {
            accent_full
        };
        let amber = if grey {
            desaturate(amber_full)
        } else {
            amber_full
        };
        let card_w = placed.size.0 * zoom;

        // Header accent wash (a filled band), only when the header is `Fill`.
        if node.header == HeaderStyle::Fill {
            if let Some(bottom) = placed.header_band_bottom() {
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

        // Header/body separator, on the header band's bottom edge. Drawn for any
        // header (`Plain` as well as `Fill`), like the web renderer's
        // `.node-hdr` border-bottom.
        if let Some(dy) = placed.header_divider() {
            self.draw_rule.color = vec4(accent.x, accent.y, accent.z, 0.22);
            self.draw_rule.draw_abs(
                cx,
                Rect {
                    pos: dvec2(screen.pos.x, screen.pos.y + dy * zoom),
                    size: dvec2(card_w, (1.0 * zoom).max(1.0)),
                },
            );
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

        // Mute the coloured text pens for an out-of-focus card; restored after the
        // loop so the next card draws full-chroma. Bold/dim are already neutral.
        if grey {
            self.draw_mono_accent.color = accent;
            self.draw_mono_amber.color = amber;
        }
        for pt in &placed.texts {
            let pos = dvec2(screen.pos.x + pt.x * zoom, screen.pos.y + pt.y * zoom);
            let size = (pt.style.size_pt * zoom) as f32; // TextStyle.font_size is f32
            let font_size = font_raster_size(size);
            let font_scale = size / font_size;
            match (pt.style.weight, pt.style.color) {
                (Weight::Bold, _) => {
                    self.draw_mono_bold.text_style.font_size = font_size;
                    self.draw_mono_bold.font_scale = font_scale;
                    self.draw_mono_bold.draw_abs(cx, pos, &pt.text);
                }
                (Weight::Regular, Token::Accent) => {
                    self.draw_mono_accent.text_style.font_size = font_size;
                    self.draw_mono_accent.font_scale = font_scale;
                    self.draw_mono_accent.draw_abs(cx, pos, &pt.text);
                }
                (Weight::Regular, Token::Amber) => {
                    self.draw_mono_amber.text_style.font_size = font_size;
                    self.draw_mono_amber.font_scale = font_scale;
                    self.draw_mono_amber.draw_abs(cx, pos, &pt.text);
                }
                (Weight::Regular, _) => {
                    self.draw_mono_dim.text_style.font_size = font_size;
                    self.draw_mono_dim.font_scale = font_scale;
                    self.draw_mono_dim.draw_abs(cx, pos, &pt.text);
                }
            }
        }
        if grey {
            self.draw_mono_accent.color = accent_full;
            self.draw_mono_amber.color = amber_full;
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

    pub fn clear(&mut self, cx: &mut Cx) {
        self.set_scene(cx, Scene::default());
    }

    pub fn set_scene(&mut self, cx: &mut Cx, scene: Scene) {
        self.reconcile_scene(cx, scene, SceneUpdate::Replace);
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
        let key = scene
            .nodes
            .first()
            .map(|node| node.key.clone())
            .unwrap_or_default();
        self.reconcile_scene(cx, scene, SceneUpdate::Focus { key });
    }

    /// Swap the scene for a same-diagram re-solve (e.g. an expand toggle). Unlike
    /// `set_scene`, this holds an already-settled camera, retargets any pending
    /// first fit to the replacement bounds, and re-resolves the selection by key,
    /// so the inspector highlight survives even though the node's index may have
    /// shifted.
    pub fn update_scene(&mut self, cx: &mut Cx, scene: Scene) {
        self.reconcile_scene(cx, scene, SceneUpdate::PreserveViewport);
    }

    /// Select the node whose key is `key` (inspector-driven navigation). Sets
    /// `selected_key` and re-resolves `selected` by key against the current
    /// scene; a key with no node in this scene (e.g. an edge) clears the
    /// selection but is otherwise a no-op. Repaints the highlight.
    pub fn select_by_key(&mut self, cx: &mut Cx, key: &str) {
        self.selection.select(key, &self.scene.nodes);
        self.draw_bg.redraw(cx);
    }

    /// Store the per-zone conflict verdict pushed by the view; repaint so the
    /// compass reddens the flagged zones on the next frame.
    /// Push the arm-time speculative solves' candidate layouts (one per zone).
    /// The same solve that produced the conflict verdicts produced these, so a
    /// hover costs no solve at all. If the cursor is already resting on a wedge
    /// when they land, latch it immediately rather than waiting for a move.
    pub fn set_zone_layouts(
        &mut self,
        cx: &mut Cx,
        layouts: Vec<(Zone, std::collections::BTreeMap<String, waml::solve::Rect>)>,
    ) {
        let effects =
            self.placement
                .set_candidate_layouts(layouts, &mut self.scene, &mut self.viewport);
        self.apply_interaction_effects(cx, effects);
    }

    /// The dial armed `zone` (or `None`: the hub / nothing armed). Drives the
    /// live layout preview. The canvas no longer hit-tests the wedges itself --
    /// the `RadialPopup` owns that, and the shell relays its arm changes here.
    pub fn preview_zone(&mut self, cx: &mut Cx, zone: Option<Zone>) {
        let effects = self
            .placement
            .preview_zone(zone, &mut self.scene, &mut self.viewport);
        self.apply_interaction_effects(cx, effects);
    }

    /// The placement `zone` would author for the live drag: the dragged
    /// (subject) node, the armed target (reference), and the direction(s).
    /// `None` when no drag/target is live. Read by the shell when the dial
    /// commits, so the committed wedge -- not the last-armed one -- decides.
    pub fn placement_for(&self, zone: Zone) -> Option<DialPlacement> {
        self.placement.placement_for(zone)
    }

    pub fn set_conflict_zones(&mut self, cx: &mut Cx, zones: Vec<Zone>) {
        if self.placement.set_conflict_zones(zones) {
            self.draw_bg.redraw(cx);
        }
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
        self.selection.set_conflict_focus_keys(keys);
        self.draw_bg.redraw(cx);
    }

    /// Set the constraint-veil visibility mode and repaint.
    pub fn set_constraint_vis(&mut self, cx: &mut Cx, mode: ConstraintVisibility) {
        self.selection.set_constraint_visibility(mode);
        self.draw_bg.redraw(cx);
    }

    /// Zoom by `factor` about the VIEWPORT CENTRE (spec §4). Deliberately
    /// unlike the scroll path, which anchors at the cursor: a button press has
    /// no cursor to honour, so holding the middle of the canvas stable is the
    /// predictable behaviour. `Camera::zoom_at` clamps to `MIN_ZOOM`/`MAX_ZOOM`;
    /// at a bound this is simply a no-op.
    /// Glides rather than snaps; the step composes onto the glide's *target*, so
    /// three presses in and three out round-trip exactly even when each lands
    /// mid-flight.
    pub fn zoom_step(&mut self, cx: &mut Cx, factor: f64) {
        let effects = self.viewport.zoom_step(factor);
        self.apply_viewport_effects(cx, effects);
    }

    /// Drive the two-finger camera gesture: the spread ratio scales the camera
    /// about the fingers' midpoint, and the midpoint's own travel pans, so a
    /// pinch can reframe and zoom in one motion. Returns true when the event
    /// belongs to a pinch, meaning the caller must not also read it as a drag.
    fn handle_pinch(&mut self, cx: &mut Cx, tu: &TouchUpdateEvent) -> bool {
        let live: Vec<&TouchPoint> = tu
            .touches
            .iter()
            .filter(|t| !matches!(t.state, TouchState::Stop))
            .collect();
        if live.len() < 2 {
            // The gesture is over. A finger still resting on the glass is
            // deliberately NOT promoted to a pan -- the user is mid-lift, and
            // the camera jumping to follow the survivor reads as a glitch.
            return self.viewport.end_pinch();
        }
        let (a, b) = (live[0], live[1]);
        let spread = a.abs.distance(&b.abs);
        let mid = (a.abs + b.abs) * 0.5;
        if self.placement.snapshot().dragged_key.is_some() {
            let effects = self.placement.cancel(&mut self.scene, &mut self.viewport);
            self.apply_interaction_effects(cx, effects);
        }
        self.viewport.suppress_release();
        let effects = self.viewport.apply_pinch_sample(TouchPair {
            a: a.uid,
            b: b.uid,
            spread,
            midpoint_abs: mid,
        });
        self.apply_viewport_effects(cx, effects);
        true
    }

    /// Frame the whole scene (spec §4). An empty scene (`bounding_box` returns
    /// `None`) or a not-yet-drawn canvas is a no-op with no camera mutation.
    /// Marks the camera as fitted so a pending one-shot load-time fit cannot
    /// stomp this on the next draw -- the user explicitly asked
    /// for a fit.
    pub fn fit_to_scene(&mut self, cx: &mut Cx) {
        let effects = self.viewport.fit_to_bounds(bounding_box(&self.scene));
        self.apply_viewport_effects(cx, effects);
    }

    /// Frame the selected node (spec §4). No selection, a key with no node in
    /// this scene, or a not-yet-drawn canvas is a no-op.
    pub fn fit_to_selection(&mut self, cx: &mut Cx) {
        let Some(key) = self.selection.selected_key().map(str::to_owned) else {
            return;
        };
        let Some(bbox) = self
            .scene
            .nodes
            .iter()
            .find(|n| n.key == key)
            .map(|n| n.rect)
        else {
            return;
        };
        let effects = self.viewport.fit_to_bounds(Some(bbox));
        self.apply_viewport_effects(cx, effects);
    }

    /// Whether a node is currently selected — drives the view bar's
    /// fit-to-selection button between enabled and dim.
    pub fn has_selection(&self) -> bool {
        self.selection.has_selection()
    }

    /// Toggle the hidden-group-border x-ray and repaint.
    pub fn set_show_hidden_borders(&mut self, cx: &mut Cx, on: bool) {
        self.selection.set_show_hidden_borders(on);
        self.draw_bg.redraw(cx);
    }

    /// Current constraint-veil mode. The canvas owns this state; the view bar's
    /// lit toggle is a mirror of it and re-syncs from here on every view
    /// `sync`.
    pub fn constraint_vis(&self) -> ConstraintVisibility {
        self.selection.snapshot().constraint_visibility
    }

    /// Current hidden-group-border x-ray state. Same ownership story as
    /// `constraint_vis`: the canvas holds it, the view bar's lit toggle mirrors
    /// it and re-syncs from here on every view `sync`.
    pub fn show_hidden_borders(&self) -> bool {
        self.selection.snapshot().show_hidden_borders
    }

    /// Node count of the current scene, for the statusbar mock.
    pub fn node_count(&self) -> usize {
        self.scene.nodes.len()
    }

    /// Convenience reader for `App` (mirrors `ToolDock::dock_action`).
    pub fn surface_action(&self, actions: &Actions) -> Option<ClassDiagramSurfaceAction> {
        let item = actions.find_widget_action(self.widget_uid())?;
        match item.cast() {
            ClassDiagramSurfaceAction::None => None,
            action => Some(action),
        }
    }

    /// Current zoom as a whole-number percentage, for the statusbar mock.
    pub fn zoom_pct(&self) -> i32 {
        (self.viewport.camera().zoom * 100.0).round() as i32
    }

    fn apply_viewport_effects(&mut self, cx: &mut Cx, effects: ViewportEffects) {
        match effects.camera_timer {
            ViewportTimerCommand::Keep => {}
            ViewportTimerCommand::StartInterval(seconds) => {
                self.cam_timer = cx.start_interval(seconds);
            }
            ViewportTimerCommand::Stop => cx.stop_timer(self.cam_timer),
        }
        if effects.redraw {
            self.draw_bg.redraw(cx);
        }
    }

    fn apply_interaction_effects(&mut self, cx: &mut Cx, effects: InteractionEffects) {
        match effects.dwell_timer {
            TimerCommand::Keep => {}
            TimerCommand::StartTimeout(seconds) => {
                self.dwell_timer = cx.start_timeout(seconds);
            }
            TimerCommand::RestartTimeout(seconds) => {
                cx.stop_timer(self.dwell_timer);
                self.dwell_timer = cx.start_timeout(seconds);
            }
            TimerCommand::Stop => cx.stop_timer(self.dwell_timer),
        }
        match effects.preview_frame {
            FrameCommand::Keep | FrameCommand::Stop => {}
            FrameCommand::Request => {
                self.preview_frame = cx.new_next_frame();
            }
        }
        if effects.redraw {
            self.draw_bg.redraw(cx);
        }
        if let Some(intent) = effects.intent {
            let action = ClassDiagramSurfaceAction::from(intent);
            cx.widget_action(self.widget_uid(), action);
        }
    }
}

fn font_raster_size(target_size: f32) -> f32 {
    if target_size <= FONT_RASTER_SIZES[0] {
        return target_size.max(4.0);
    }

    FONT_RASTER_SIZES
        .iter()
        .copied()
        .min_by(|a, b| {
            (target_size - *a)
                .abs()
                .total_cmp(&(target_size - *b).abs())
                .then_with(|| b.total_cmp(a))
        })
        .unwrap_or(target_size)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::canvas::class::placement::{
        lerp_rect, preview_zoom, zone_id, zone_of_id, DIAL_ZONES,
    };
    use crate::canvas::viewport::ease_out;

    use waml::solve::Rect as WorldRect;

    #[test]
    fn every_surface_intent_maps_one_to_one_to_a_widget_action() {
        let cases = [
            (
                SurfaceIntent::NodeMenu {
                    abs: dvec2(12.0, 34.0),
                    key: "menu-key".into(),
                },
                ClassDiagramSurfaceAction::NodeMenu {
                    abs: dvec2(12.0, 34.0),
                    key: "menu-key".into(),
                },
            ),
            (
                SurfaceIntent::NodeSelect {
                    key: "selected-key".into(),
                },
                ClassDiagramSurfaceAction::NodeSelect {
                    key: "selected-key".into(),
                },
            ),
            (
                SurfaceIntent::NodeDeselect,
                ClassDiagramSurfaceAction::NodeDeselect,
            ),
            (
                SurfaceIntent::ToggleExpand {
                    key: "expanded-key".into(),
                },
                ClassDiagramSurfaceAction::ToggleExpand {
                    key: "expanded-key".into(),
                },
            ),
            (
                SurfaceIntent::CompassArmed {
                    subject_key: "subject".into(),
                    reference_key: "reference".into(),
                    center: dvec2(400.0, 300.0),
                },
                ClassDiagramSurfaceAction::CompassArmed {
                    subject_key: "subject".into(),
                    reference_key: "reference".into(),
                    center: dvec2(400.0, 300.0),
                },
            ),
            (
                SurfaceIntent::DialDismiss,
                ClassDiagramSurfaceAction::DialDismiss,
            ),
        ];
        for (intent, expected) in cases {
            assert_eq!(ClassDiagramSurfaceAction::from(intent), expected);
        }
    }

    #[test]
    fn font_raster_size_keeps_small_text_exact() {
        assert_eq!(font_raster_size(4.0), 4.0);
        assert_eq!(font_raster_size(17.25), 17.25);
        assert_eq!(font_raster_size(32.0), 32.0);
    }

    #[test]
    fn font_raster_size_selects_the_nearest_ladder_rung() {
        assert_eq!(font_raster_size(33.0), 32.0);
        assert_eq!(font_raster_size(39.0), 40.0);
        assert_eq!(font_raster_size(61.0), 63.0);
        assert_eq!(font_raster_size(100.0), 99.0);
    }

    #[test]
    fn font_raster_size_resolves_midpoints_upward_and_caps_at_the_largest_rung() {
        assert_eq!(font_raster_size(36.0), 40.0);
        assert_eq!(font_raster_size(44.0), 40.0);
        assert_eq!(font_raster_size(45.0), 50.0);
        assert_eq!(font_raster_size(400.0), 304.0);
    }

    /// Shader-shape gate for the `GroupDashed` pen's dash mask. The pixel fn
    /// cannot be executed headless (it needs a `Cx`/GPU), so the only place an
    /// asymmetric duty mask can be caught is a source assertion here.
    ///
    /// A mask built as `clamp((0.5 - f) * dash_px, 0, 1)` filters only the
    /// TRAILING edge of each dash: at the wrap point (`fract` rolls 1 -> 0) the
    /// term jumps straight from a large negative to +0.5*dash_px, so the
    /// leading edge is a hard step that crawls and shimmers under pan/zoom.
    /// The fix is the symmetric triangle-wave distance `abs(fract(..) - 0.5)`
    /// that `ConstraintVeil` already uses, which filters both edges alike.
    #[test]
    fn the_dash_mask_filters_both_edges_symmetrically() {
        let src = include_str!("widget.rs");
        let pen = src
            .split_once("mod.draw.GroupDashed = ")
            .and_then(|(_, rest)| rest.split_once("\n    }\n"))
            .map(|(body, _)| body)
            .expect("widget.rs must define the `mod.draw.GroupDashed` pen");
        // Drop the pen's own comments -- they name the broken idiom on purpose.
        let pen: String = pen
            .lines()
            .filter(|l| !l.trim_start().starts_with("//"))
            .collect::<Vec<_>>()
            .join("\n");

        assert!(
            pen.contains("abs(fract("),
            "the GroupDashed dash mask must take its distance from the \
             symmetric `abs(fract(..) - 0.5)` triangle wave (the ConstraintVeil \
             idiom) so BOTH dash edges are antialiased"
        );
        assert!(
            !pen.contains("(0.5 - f)"),
            "`(0.5 - f)` is the one-sided ramp that leaves the leading dash \
             edge a hard step (it crawls under pan/zoom); use the symmetric \
             `abs(fract(..) - 0.5)` distance instead"
        );
    }

    #[test]
    fn only_frame_groups_draw_chrome() {
        use waml::syntax::Shape;
        // Frame always draws its chrome, x-ray or not.
        assert_eq!(group_draw_mode(Shape::Frame, false), GroupDraw::Chrome);
        assert_eq!(group_draw_mode(Shape::Frame, true), GroupDraw::Chrome);
        // Box/Shrink are layout-only: invisible by default...
        assert_eq!(group_draw_mode(Shape::Box, false), GroupDraw::Skip);
        assert_eq!(group_draw_mode(Shape::Shrink, false), GroupDraw::Skip);
        // ...and dashed under the hidden-borders x-ray.
        assert_eq!(group_draw_mode(Shape::Box, true), GroupDraw::Dashed);
        assert_eq!(group_draw_mode(Shape::Shrink, true), GroupDraw::Dashed);
    }

    #[test]
    fn untitled_groups_get_a_one_based_label() {
        assert_eq!(untitled_label(1), "Untitled 1");
        assert_eq!(untitled_label(2), "Untitled 2");
        assert_eq!(untitled_label(12), "Untitled 12");
    }

    /// A `SolvedGroup` with just the two fields the render gating reads.
    fn group(title: Option<&str>, shape: waml::syntax::Shape) -> waml::solve::SolvedGroup {
        waml::solve::SolvedGroup {
            rect: WorldRect {
                x: 0.0,
                y: 0.0,
                w: 10.0,
                h: 10.0,
            },
            shape,
            title: title.map(|s| s.to_string()),
            depth: 0,
        }
    }

    #[test]
    fn a_named_group_always_draws_its_own_name() {
        // Named groups keep their title in every mode -- the `Untitled N`
        // fallback must never win over an authored name.
        for mode in [GroupDraw::Chrome, GroupDraw::Dashed] {
            assert_eq!(
                group_label(&Some("Users".to_string()), None, mode),
                Some("Users".to_string()),
                "{mode:?} must draw the authored name"
            );
        }
        // An unnamed `frame` group draws no title, exactly as before the x-ray.
        assert_eq!(group_label(&None, Some(1), GroupDraw::Chrome), None);
        // Only the dashed x-ray outline gets the fallback.
        assert_eq!(
            group_label(&None, Some(3), GroupDraw::Dashed),
            Some("Untitled 3".to_string())
        );
        // No ordinal (i.e. a named group) means no fallback either way.
        assert_eq!(group_label(&None, None, GroupDraw::Dashed), None);
    }

    #[test]
    fn the_plan_covers_every_group_in_scene_order() {
        use waml::syntax::Shape;
        let groups = [
            group(Some("Users"), Shape::Frame),
            group(None, Shape::Box),
            group(Some("Billing"), Shape::Shrink),
        ];
        // X-ray off: only the `frame` draws; the two layout-only groups are
        // skipped but still occupy their slot so callers can zip by index.
        let plan = group_plan(&groups, false);
        assert_eq!(
            plan.len(),
            groups.len(),
            "one entry per group, Skip included"
        );
        assert_eq!(plan[0], (GroupDraw::Chrome, Some("Users".to_string())));
        assert_eq!(plan[1].0, GroupDraw::Skip);
        assert_eq!(plan[2].0, GroupDraw::Skip);
    }

    #[test]
    fn the_untitled_counter_advances_over_groups_that_draw_no_title() {
        use waml::syntax::Shape;
        // An unnamed `frame` (drawn, but titleless) leads, then a `box` pair
        // straddling a named group. The counter must advance past the frame --
        // it burns ordinal 1 without ever showing it -- so the first x-rayed
        // group reads `Untitled 2`, not `Untitled 1`.
        let groups = [
            group(None, Shape::Frame),
            group(None, Shape::Box),
            group(Some("Billing"), Shape::Box),
            group(None, Shape::Box),
        ];
        let off = group_plan(&groups, false);
        assert_eq!(off[0], (GroupDraw::Chrome, None));
        assert_eq!(off[1].0, GroupDraw::Skip);

        let on = group_plan(&groups, true);
        assert_eq!(
            on[0],
            (GroupDraw::Chrome, None),
            "an unnamed frame stays titleless under the x-ray"
        );
        assert_eq!(
            on[1],
            (GroupDraw::Dashed, Some("Untitled 2".to_string())),
            "the titleless frame must still consume ordinal 1"
        );
        assert_eq!(on[2], (GroupDraw::Dashed, Some("Billing".to_string())));
        assert_eq!(
            on[3],
            (GroupDraw::Dashed, Some("Untitled 3".to_string())),
            "a named group must not consume an untitled ordinal"
        );
    }

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

    /// The wedge `RadialPopup` would resolve at `p`, mapped through the dial's
    /// zone table -- i.e. exactly what the shell relays back into
    /// `preview_zone`. Tests the seam between the widget's geometry and this
    /// module's ordering; the widget owns the hit-test itself.
    fn dial_pick(center: DVec2, p: DVec2) -> Option<Zone> {
        crate::popup::radial::RadialLayout::full(DIAL_ZONES.len())
            .index_at(center, p)
            .map(|i| DIAL_ZONES[i])
    }

    #[test]
    fn dial_wedges_follow_the_radial_clock() {
        let c = dvec2(500.0, 400.0);
        // Wedge 0 is centred on 12 o'clock and the ring runs clockwise, so the
        // dial's zone order has to read the same way round.
        assert_eq!(dial_pick(c, dvec2(500.0, 400.0 - 60.0)), Some(Zone::Top));
        assert_eq!(dial_pick(c, dvec2(500.0 + 60.0, 400.0)), Some(Zone::Right));
        assert_eq!(dial_pick(c, dvec2(500.0, 400.0 + 60.0)), Some(Zone::Bottom));
        assert_eq!(dial_pick(c, dvec2(500.0 - 60.0, 400.0)), Some(Zone::Left));
        assert_eq!(
            dial_pick(c, dvec2(500.0 + 42.0, 400.0 - 42.0)),
            Some(Zone::TopRight)
        );
    }

    #[test]
    fn dial_hub_is_dead_and_overshoot_still_lands() {
        let c = dvec2(500.0, 400.0);
        let hub = crate::popup::radial::HUB_RADIUS;
        let rim = crate::popup::radial::DISC_RADIUS;
        // Inside the hub: no zone, so releasing on the target's own body cancels.
        assert_eq!(dial_pick(c, dvec2(500.0, 400.0 - hub * 0.5)), None);
        // Past the rim the pick is angle-only: an overshot flick still counts,
        // and the drag only gives up on the dial past DIAL_REACH.
        assert_eq!(
            dial_pick(c, dvec2(500.0, 400.0 - rim * 3.0)),
            Some(Zone::Top)
        );
    }

    #[test]
    fn every_wedge_id_round_trips_to_its_zone() {
        // Ids (not slot indices) cross the popup seam, so a commit can only be
        // mapped back if the table is injective.
        for z in DIAL_ZONES {
            assert_eq!(zone_of_id(zone_id(z)), Some(z));
        }
        assert_eq!(zone_of_id(live_id!(not_a_wedge)), None);
    }

    #[test]
    fn preview_zoom_fits_both_nodes_and_never_magnifies_much() {
        let a = waml::solve::Rect {
            x: 0.0,
            y: 0.0,
            w: 100.0,
            h: 100.0,
        };
        let far = waml::solve::Rect {
            x: 1900.0,
            y: 0.0,
            w: 100.0,
            h: 100.0,
        };
        let view = dvec2(1000.0, 700.0);
        // Spread apart: zoom out to fit the 2000px span in 1000-2*72.
        let z = preview_zoom(a, far, view, 72.0, 1.0);
        assert!((z - (1000.0 - 144.0) / 2000.0).abs() < 1e-9, "{z}");
        // Adjacent: the fit would magnify hugely, but a preview is capped at a
        // quarter-step past where the drag started, and never past 1:1.
        let near = waml::solve::Rect {
            x: 120.0,
            y: 0.0,
            w: 100.0,
            h: 100.0,
        };
        assert_eq!(preview_zoom(a, near, view, 72.0, 0.4), 0.5);
        assert_eq!(preview_zoom(a, near, view, 72.0, 2.0), 1.0);
    }

    #[test]
    fn tween_helpers_hit_their_endpoints() {
        assert_eq!(ease_out(0.0), 0.0);
        assert_eq!(ease_out(1.0), 1.0);
        // Ease-out: past the halfway mark by the halfway frame.
        assert!(ease_out(0.5) > 0.5);
        let a = waml::solve::Rect {
            x: 0.0,
            y: 10.0,
            w: 4.0,
            h: 8.0,
        };
        let b = waml::solve::Rect {
            x: 10.0,
            y: 30.0,
            w: 8.0,
            h: 8.0,
        };
        assert_eq!(lerp_rect(a, b, 0.0), a);
        assert_eq!(lerp_rect(a, b, 1.0), b);
        let m = lerp_rect(a, b, 0.5);
        assert_eq!((m.x, m.y, m.w, m.h), (5.0, 20.0, 6.0, 8.0));
    }

    #[test]
    fn veil_band_anchors_and_clamps_per_direction() {
        // reference screen rect, reach.
        let reference = Rect {
            pos: dvec2(200.0, 100.0), // x span 200..380, y span 100..180
            size: dvec2(180.0, 80.0),
        };
        let reach = 300.0;
        use waml::syntax::Direction::*;

        // left of: band starts at the reference LEFT edge, extends right `reach`;
        // the y (cross) axis is bounded to the reference span grown by `reach`
        // each side (100-300 .. 180+300), centred on the reference -- no longer
        // the full view.
        let b = veil_band(reference, LeftOf, reach);
        assert_eq!(b.pos.x, 200.0);
        assert_eq!(b.size.x, 300.0);
        assert_eq!(b.pos.y, -200.0);
        assert_eq!(b.size.y, 680.0);

        // right of: band ends at the reference RIGHT edge (380), extends left `reach`.
        let b = veil_band(reference, RightOf, reach);
        assert_eq!(b.pos.x + b.size.x, 380.0);
        assert_eq!(b.size.x, 300.0);
        assert_eq!(b.pos.y, -200.0);
        assert_eq!(b.size.y, 680.0);

        // above: band starts at the reference TOP edge, extends down `reach`; the
        // x (cross) axis is the reference span grown by `reach` each side.
        let b = veil_band(reference, Above, reach);
        assert_eq!(b.pos.y, 100.0);
        assert_eq!(b.size.y, 300.0);
        assert_eq!(b.pos.x, -100.0);
        assert_eq!(b.size.x, 780.0);

        // above left of: BOTH axes locked to reach off the top-left corner.
        let b = veil_band(reference, AboveLeft, reach);
        assert_eq!(
            (b.pos.x, b.pos.y, b.size.x, b.size.y),
            (200.0, 100.0, 300.0, 300.0)
        );
    }

    #[test]
    fn cross_fade_centres_on_the_reference_on_the_unlocked_axis() {
        use waml::syntax::Direction::*;
        let reference = Rect {
            pos: dvec2(200.0, 100.0), // y span 100..180, centre 140, half 40
            size: dvec2(180.0, 80.0),
        };
        let reach = 300.0;
        let close = |a: f32, b: f32| (a - b).abs() < 1e-4;

        // LeftOf: y unlocked. Band is the reference y-span grown by reach each
        // side (height 680), so the reference sits dead-centre -> ctr 0.5. The
        // plateau (opaque core) is the reference half-span and the soft tail is
        // `reach`, both normalized to the band height.
        let band = veil_band(reference, LeftOf, reach);
        let (ctr, plateau, soft) = cross_fade_params(band, reference, LeftOf, reach);
        // x axis is locked -> the flat "always 1" triple.
        assert_eq!((ctr[0], plateau[0], soft[0]), (0.5, 2.0, 1.0));
        // y axis carries the real fade.
        assert!(close(ctr[1], 0.5));
        assert!(close(plateau[1], 40.0 / 680.0));
        assert!(close(soft[1], 300.0 / 680.0));

        // Diagonals lock both axes -> flat on both (extend fade shapes them).
        let band = veil_band(reference, AboveLeft, reach);
        let (ctr, plateau, soft) = cross_fade_params(band, reference, AboveLeft, reach);
        assert_eq!((ctr[0], plateau[0], soft[0]), (0.5, 2.0, 1.0));
        assert_eq!((ctr[1], plateau[1], soft[1]), (0.5, 2.0, 1.0));
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
        // The default is `Selected` -- the bar's constraints toggle starts ON.
        assert_eq!(
            ConstraintVisibility::default(),
            ConstraintVisibility::Selected
        );
    }

    #[test]
    fn focus_state_splits_selected_neighbour_and_outsider() {
        use std::collections::HashSet;
        // Focus set: the picked node `order` plus its two constraint neighbours.
        let focus: HashSet<String> = ["order", "payment-gateway", "user"]
            .into_iter()
            .map(String::from)
            .collect();
        // The picked node: `selected`, not `related`, coloured.
        let sel = node_focus_state("order", Some("order"), &focus);
        assert_eq!(
            sel,
            FocusState {
                selected: true,
                related: false
            }
        );
        assert!(sel.coloured());
        // A neighbour: `related`, not `selected`, coloured.
        let nbr = node_focus_state("payment-gateway", Some("order"), &focus);
        assert_eq!(
            nbr,
            FocusState {
                selected: false,
                related: true
            }
        );
        assert!(nbr.coloured());
        // An outsider (not in the focus set): neither -> greyscale.
        let out = node_focus_state("archive", Some("order"), &focus);
        assert_eq!(out, FocusState::default());
        assert!(!out.coloured());
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
    fn clear_delegates_to_the_existing_empty_scene_reset() {
        let _: fn(&mut ClassDiagramSurface, &mut Cx) = ClassDiagramSurface::clear;
        let src = include_str!("widget.rs");
        let body = src
            .split_once("    pub fn clear(&mut self, cx: &mut Cx) {")
            .and_then(|(_, rest)| rest.split_once("\n    }"))
            .map(|(body, _)| body.trim())
            .expect("ClassDiagramSurface must expose clear");
        assert_eq!(body, "self.set_scene(cx, Scene::default());");
    }

    mod reconciliation {
        use super::*;

        fn surface_with_live_dial(vm: &mut ScriptVm) -> ClassDiagramSurface {
            use waml::model::{ElementType, UmlMetaclass};

            let mut surface = ClassDiagramSurface::script_new(vm);
            let node = |key: &str, title: &str, x: f64| crate::scene::SceneNode {
                key: key.into(),
                title: title.into(),
                element_type: ElementType::Uml(UmlMetaclass::Class),
                stereotypes: Vec::new(),
                attributes: Vec::new(),
                operations: Vec::new(),
                header: crate::scene::HeaderStyle::Plain,
                ports: false,
                rect: WorldRect {
                    x,
                    y: 0.0,
                    w: 80.0,
                    h: 60.0,
                },
                emphasized: false,
                collapsed: false,
                expanded: false,
            };
            surface.scene.nodes = vec![
                node("old-subject", "Old subject", 0.0),
                node("old-reference", "Old reference", 120.0),
            ];
            surface
                .placement
                .begin_drag("old-subject", dvec2(10.0, 10.0), (10.0, 10.0));
            surface
                .placement
                .hover_target(Some("old-reference"), &surface.scene);
            surface
                .placement
                .dwell_elapsed(&surface.scene, dvec2(100.0, 100.0));
            surface
        }

        fn assert_scene_update_invalidates_old_dial_identity(update: SceneUpdate) {
            let mut vm = crate::script_gate::boot_test_vm();
            let mut surface = surface_with_live_dial(&mut vm);

            let live = surface
                .placement_for(Zone::Top)
                .expect("an ordinary live dial must still resolve its placement");
            assert_eq!(live.subject_key, "old-subject");
            assert_eq!(live.reference_key, "old-reference");

            match update {
                SceneUpdate::Replace => surface.set_scene(vm.cx_mut(), Scene::default()),
                SceneUpdate::Focus { .. } => surface.set_focus(vm.cx_mut(), Scene::default()),
                SceneUpdate::PreserveViewport => {
                    surface.update_scene(vm.cx_mut(), Scene::default())
                }
            }

            assert!(surface.placement_for(Zone::Top).is_none());
        }

        #[test]
        fn replace_clears_selection_and_refits() {
            assert_eq!(
                reconciliation_policy(&SceneUpdate::Replace),
                ReconciliationPolicy {
                    clear_placement: true,
                    selection: SelectionPolicy::Clear,
                    camera: CameraPolicy::Refit,
                },
            );
        }

        #[test]
        fn focus_preserves_the_unselected_preview_behavior() {
            let policy = reconciliation_policy(&SceneUpdate::Focus {
                key: "order".into(),
            });
            assert_eq!(policy.selection, SelectionPolicy::Clear);
            assert_eq!(policy.camera, CameraPolicy::Focus);
        }

        #[test]
        fn update_scene_preserves_camera_and_re_resolves_selection() {
            assert_eq!(
                reconciliation_policy(&SceneUpdate::PreserveViewport),
                ReconciliationPolicy {
                    clear_placement: true,
                    selection: SelectionPolicy::Preserve,
                    camera: CameraPolicy::Retain,
                },
            );
        }

        #[test]
        fn every_scene_update_invalidates_the_old_dial_identity() {
            assert_scene_update_invalidates_old_dial_identity(SceneUpdate::Replace);
            assert_scene_update_invalidates_old_dial_identity(SceneUpdate::Focus {
                key: "ignored-by-public-set-focus".into(),
            });
            assert_scene_update_invalidates_old_dial_identity(SceneUpdate::PreserveViewport);
        }
    }
}
