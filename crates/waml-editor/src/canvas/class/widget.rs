//! The `ClassDiagramSurface` widget: draws the flattened `Scene` under a pan/zoom
//! `Camera`. Read-only — no editing, no hit-testing of individual nodes.
//! Fits the scene to the view on first draw; left-drag pans; scroll zooms
//! toward the cursor. Each node is a filled rect + its title text.
//!
//! Structure/hit-handling mirror the fork's `widgets/src/map/view.rs`.

use super::{
    interaction::ClassInteraction,
    placement::PlacementInteraction,
    render::{self, ClassDrawResources, LineworkMetrics, RenderSnapshot, DEFAULT_LINEWORK_MODE},
    selection::{ConstraintVisibility, SelectionPolicy, SelectionState, SELECTION_TICK},
    DialPlacement, FrameCommand, InteractionEffects, SceneUpdate, SurfaceIntent, TimerCommand,
    Zone,
};
use crate::canvas::viewport::{
    InitialFit, TimerCommand as ViewportTimerCommand, TouchPair, ViewportController,
    ViewportEffects,
};
use crate::inspector::Subject;
use crate::popup::base::PopupItem;
use crate::scene::{bounding_box, Scene};
use makepad_widgets::event::{TouchPoint, TouchState, TouchUpdateEvent};
use makepad_widgets::*;

const STALE_BADGE_LABEL: &str = "STALE";
const STALE_BADGE_WIDTH: f64 = 58.0;
const STALE_BADGE_HEIGHT: f64 = 24.0;
const STALE_BADGE_INSET: f64 = 12.0;

fn stale_badge_rect(view_rect: Rect) -> Rect {
    Rect {
        pos: dvec2(
            view_rect.pos.x + view_rect.size.x - STALE_BADGE_WIDTH - STALE_BADGE_INSET,
            view_rect.pos.y + STALE_BADGE_INSET,
        ),
        size: dvec2(STALE_BADGE_WIDTH, STALE_BADGE_HEIGHT),
    }
}

fn draw_stale_badge_overlay(
    cx: &mut Cx2d,
    view_rect: Rect,
    badge: &mut DrawColor,
    text: &mut DrawText,
) {
    let rect = stale_badge_rect(view_rect);
    badge.draw_abs(cx, rect);
    let size = text
        .layout(
            cx,
            0.0,
            0.0,
            None,
            false,
            Align::default(),
            STALE_BADGE_LABEL,
        )
        .size_in_lpxs;
    text.draw_abs(
        cx,
        dvec2(
            rect.pos.x + (rect.size.x - size.width as f64) * 0.5,
            rect.pos.y + (rect.size.y - size.height as f64) * 0.5,
        ),
        STALE_BADGE_LABEL,
    );
}

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

    // Edge corner shader: rasterizes the quad-local geometry from
    // `corner_fillet` as ONE combined SDF. The pixel fn UNIONS the two short bar
    // stubs (`bar_in`/`bar_out`) and quarter-arc band, so their joints are
    // interior to one filled shape: solid, with AA only on the outer boundary.
    // Fades text_dim -> text zoomed out like `EdgeLine` so a corner never reads
    // brighter than the bars it joins.
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

    // Edge end-adornment shader: consumes `marker_geometry`'s four quad-local
    // path vertices in `v01`/`v23`. The shader is branch-free: an `if` on a
    // uniform silently no-ops in this fork's shader VM (see `action_link`), so
    // fill vs hollow vs open is selected by the `hollow`/`filled` flags
    // multiplying colors -- open (both 0) -> transparent interior + stroke,
    // hollow -> `bg` interior + stroke, filled -> `color` interior + stroke.
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
    // per-side stamping. Dash period and stroke width are pushed from the active
    // linework mode; CAD keeps both fixed in screen space. Branch-free: an `if`
    // on a uniform silently no-ops in this fork's shader VM (see EdgeMarker
    // above), so the on/off duty is a 0..1 mask multiplied into the stroke alpha.
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
    // Frame-group border. `group_fill` sits one step off the canvas ground, so a
    // fill-only frame reads as almost nothing; this solid hairline gives the
    // frame an edge. Same inset-rect stroke as `GroupDashed` below, minus the
    // duty mask. `sdf.rect`, never `sdf.box(.., 0)` -- a zero radius floods.
    mod.draw.GroupBorder = mod.draw.DrawColor{
        stroke_w: uniform(1.0)
        pixel: fn() {
            let p = self.pos * self.rect_size
            let sdf = Sdf2d.viewport(p)
            let inset = self.stroke_w * 0.5
            sdf.rect(inset, inset, self.rect_size.x - inset * 2.0, self.rect_size.y - inset * 2.0)
            sdf.stroke(self.color, self.stroke_w)
            return sdf.result
        }
    }

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
        // Frame-group edge; dim ink, same secondary-chrome weight as the dashed
        // x-ray outline so both group kinds read as one family.
        draw_group_border: mod.draw.GroupBorder{ color: atlas.text_dim }
        // Hidden-group x-ray outline; dim ink so it reads as secondary chrome.
        draw_group_dashed: mod.draw.GroupDashed{ color: atlas.text_dim }
        // Colour-only holder (never drawn): the dim ink copied onto `draw_text`
        // for a hidden group's title, so no RGBA crosses Rust.
        draw_group_title_dim +: { color: atlas.text_dim }
        // Node card: the shared node material preset. Only its fill differs.
        draw_node: mod.draw.NodeSurface{ color: atlas.field_bg }
        draw_edge_down: mod.draw.EdgeLine{ color: atlas.text_dim }
        // Rounded-corner pen; shares the edge line color so a fillet reads as part
        // of the same stroke.
        draw_elbow: mod.draw.EdgeElbow{ color: atlas.text_dim }
        // Terminal adornment pen; shares the edge line color so glyphs read as
        // part of the same stroke.
        draw_marker: mod.draw.EdgeMarker{ color: atlas.text_dim }
        // Opaque label chip keeps terminal text legible over crossed edge runs.
        // It takes the CANVAS ground, not `field_bg`: a lighter-than-canvas chip
        // reads as a widget on the diagram instead of a hole punched in the edge.
        draw_edge_label_bg +: { color: atlas.canvas_ground }
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
        // `text_mid`, not `text_dim`: at the 8px annotation size a dim grey
        // drops below legibility, while full `text` competes with the cards.
        draw_edge_label +: {
            color: atlas.text_mid
            text_style: TextStyle{
                font_size: 11
                font_family: FontFamily{
                    latin := FontMember{res: crate_resource("self:resources/fonts/IBM_Plex_Mono/IBMPlexMono-Regular.ttf") asc: -0.1 desc: 0.0}
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
        draw_stale_badge +: { color: atlas.bucket_amber }
        draw_stale_text +: {
            color: atlas.canvas_ground
            text_style: TextStyle{
                font_size: 11
                font_family: FontFamily{
                    latin := FontMember{res: crate_resource("self:resources/fonts/IBM_Plex_Mono/IBMPlexMono-Bold.ttf") asc: -0.1 desc: 0.0}
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
    draw_group_border: DrawColor,
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
    draw_edge_label_bg: DrawColor,
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
    draw_edge_label: DrawText,
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
    #[redraw]
    #[live]
    draw_stale_badge: DrawColor,
    #[redraw]
    #[live]
    draw_stale_text: DrawText,

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
    /// Animation clock for the render-only node selection lift.
    #[rust]
    selection_timer: Timer,
    /// Makepad handle for the controller-owned dwell timeout.
    #[rust]
    dwell_timer: Timer,
    #[rust]
    selection: SelectionState,
    #[rust]
    projection_stale: bool,
    #[live(true)]
    interaction_enabled: bool,
}

fn should_handle_surface_event(interaction_enabled: bool, event: &Event) -> bool {
    interaction_enabled
        || !matches!(
            event,
            Event::MouseDown(_)
                | Event::MouseMove(_)
                | Event::MouseUp(_)
                | Event::MouseLeave(_)
                | Event::TouchUpdate(_)
                | Event::LongPress(_)
                | Event::Scroll(_)
                | Event::KeyDown(_)
        )
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
        if !should_handle_surface_event(self.interaction_enabled, event) {
            return;
        }
        if let Some(te) = self.cam_timer.is_event(event) {
            let effects = self.viewport.tick_camera(te.time.unwrap_or(0.0));
            self.apply_viewport_effects(cx, effects);
        }
        if let Some(te) = self.selection_timer.is_event(event) {
            self.selection.tick_lift(te.time.unwrap_or(0.0));
            self.draw_bg.redraw(cx);
            if !self.selection.lift_animation_active() {
                cx.stop_timer(self.selection_timer);
            }
        }
        if let Event::KeyDown(ke) = event {
            if ke.key_code == KeyCode::Escape && self.placement.snapshot().dragged_key.is_some() {
                let effects = self.cancel_active_placement();
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
        self.sync_selection_lift_timer(cx);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, _scope: &mut Scope, walk: Walk) -> DrawStep {
        let rect = cx.walk_turtle(walk);
        let Self {
            scene,
            viewport,
            placement,
            selection,
            draw_bg,
            draw_node,
            draw_group,
            draw_group_border,
            draw_group_dashed,
            draw_group_title_dim,
            draw_edge_down,
            draw_elbow,
            draw_marker,
            draw_edge_label_bg,
            draw_rule,
            draw_veil,
            draw_text,
            draw_edge_label,
            draw_mono_dim,
            draw_mono_bold,
            draw_mono_accent,
            draw_mono_amber,
            draw_stale_badge,
            draw_stale_text,
            projection_stale,
            ..
        } = self;

        viewport.set_view_rect(rect);
        viewport.apply_initial_fit();
        let viewport = viewport.snapshot();
        let snapshot = RenderSnapshot {
            scene,
            linework: LineworkMetrics::for_zoom(DEFAULT_LINEWORK_MODE, viewport.camera.zoom),
            viewport,
            selection: selection.snapshot(),
            placement: placement.snapshot(),
        };
        let mut draws = ClassDrawResources {
            bg: draw_bg,
            node: draw_node,
            group: draw_group,
            group_border: draw_group_border,
            group_dashed: draw_group_dashed,
            group_title_dim: draw_group_title_dim,
            edge: draw_edge_down,
            elbow: draw_elbow,
            marker: draw_marker,
            edge_label_bg: draw_edge_label_bg,
            rule: draw_rule,
            veil: draw_veil,
            text: draw_text,
            edge_label: draw_edge_label,
            mono_dim: draw_mono_dim,
            mono_bold: draw_mono_bold,
            mono_accent: draw_mono_accent,
            mono_amber: draw_mono_amber,
        };
        render::draw(cx, &snapshot, &mut draws);
        if *projection_stale {
            draw_stale_badge_overlay(cx, viewport.view_rect, draw_stale_badge, draw_stale_text);
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
    pub(crate) fn set_projection_stale(&mut self, cx: &mut Cx, stale: bool) {
        if self.projection_stale != stale {
            self.projection_stale = stale;
            self.draw_bg.redraw(cx);
        }
    }

    #[cfg(test)]
    fn projection_stale(&self) -> bool {
        self.projection_stale
    }

    /// Enable or disable every raw camera/selection/placement interaction.
    /// Disabling cancels in-flight animation and placement without changing
    /// the settled camera or current selection.
    pub fn set_interaction_enabled(&mut self, cx: &mut Cx, enabled: bool) {
        if self.interaction_enabled == enabled {
            return;
        }
        self.interaction_enabled = enabled;
        if !enabled {
            let viewport_effects = self.viewport.cancel_glide();
            self.apply_viewport_effects(cx, viewport_effects);
            let placement_effects = self.cancel_active_placement();
            self.apply_interaction_effects(cx, placement_effects);
        }
    }

    fn cancel_active_placement(&mut self) -> InteractionEffects {
        let effects = self.placement.cancel(&mut self.scene, &mut self.viewport);
        self.viewport.end_pan();
        effects
    }

    fn prepare_scene_reconciliation(
        &mut self,
        scene: &Scene,
        update: &SceneUpdate,
    ) -> (InteractionEffects, ViewportEffects) {
        let policy = reconciliation_policy(update);
        let interaction_effects = if policy.clear_placement {
            let effects = self
                .placement
                .cancel_for_scene_change(&mut self.scene, &mut self.viewport);
            self.viewport.end_pan();
            effects
        } else {
            InteractionEffects::default()
        };

        self.viewport.end_pinch();
        let mut viewport_effects = self.viewport.cancel_glide();
        let bounds = bounding_box(scene);
        self.selection.reconcile(&scene.nodes, policy.selection);

        match (update, policy.camera) {
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
                viewport_effects = self.viewport.retain_for_scene_update(bounds);
            }
            _ => unreachable!("reconciliation policy must match its scene update"),
        }

        (interaction_effects, viewport_effects)
    }

    fn reconcile_scene(&mut self, cx: &mut Cx, scene: Scene, update: SceneUpdate) {
        let (interaction_effects, viewport_effects) =
            self.prepare_scene_reconciliation(&scene, &update);
        self.apply_interaction_effects(cx, interaction_effects);
        self.apply_viewport_effects(cx, viewport_effects);
        self.scene = scene;
        self.sync_selection_lift_timer(cx);
        self.draw_bg.redraw(cx);
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
        self.sync_selection_lift_timer(cx);
        self.draw_bg.redraw(cx);
    }

    pub fn clear_selection(&mut self, cx: &mut Cx) {
        self.selection.clear();
        self.sync_selection_lift_timer(cx);
        self.draw_bg.redraw(cx);
    }

    pub fn selected_key(&self) -> Option<&str> {
        self.selection.selected_key()
    }

    pub fn camera_anchor(&self) -> crate::view_history::DiagramCameraAnchor {
        let camera = self.viewport.snapshot().camera;
        crate::view_history::DiagramCameraAnchor {
            pan_x: camera.pan_x,
            pan_y: camera.pan_y,
            zoom: camera.zoom,
        }
    }

    pub fn restore_camera_anchor(
        &mut self,
        cx: &mut Cx,
        anchor: crate::view_history::DiagramCameraAnchor,
    ) {
        self.viewport
            .restore_camera(crate::canvas::viewport::Camera {
                pan_x: anchor.pan_x,
                pan_y: anchor.pan_y,
                zoom: anchor.zoom,
            });
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
            let effects = self.cancel_active_placement();
            self.apply_interaction_effects(cx, effects);
        }
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

    fn sync_selection_lift_timer(&mut self, cx: &mut Cx) {
        if !self.selection.take_lift_target_changed() {
            return;
        }
        cx.stop_timer(self.selection_timer);
        if self.selection.lift_animation_active() {
            self.selection_timer = cx.start_interval(SELECTION_TICK);
        }
        self.draw_bg.redraw(cx);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::canvas::class::placement::{
        lerp_rect, preview_zoom, zone_id, zone_of_id, DIAL_ZONES,
    };
    use crate::canvas::class::zone_placed;
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
    fn corner_zones_author_a_single_diagonal_direction() {
        use waml::layout::Direction::*;
        assert_eq!(zone_placed(Zone::TopLeft).dir, Some(AboveLeft));
        assert_eq!(zone_placed(Zone::TopRight).dir, Some(AboveRight));
        assert_eq!(zone_placed(Zone::BottomLeft).dir, Some(BelowLeft));
        assert_eq!(zone_placed(Zone::BottomRight).dir, Some(BelowRight));
    }

    #[test]
    fn edge_zones_author_a_single_cardinal_direction() {
        use waml::layout::Direction::*;
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

    #[test]
    fn hidden_surface_rejects_real_raw_scroll_input() {
        use makepad_widgets::event::{ScrollEvent, ScrollPhase};
        use std::cell::Cell;

        let scroll = Event::Scroll(ScrollEvent {
            window_id: WindowId(0, 0),
            scroll: dvec2(0.0, 120.0),
            abs: dvec2(640.0, 420.0),
            modifiers: KeyModifiers::default(),
            handled_x: Cell::new(false),
            handled_y: Cell::new(false),
            is_mouse: true,
            time: 0.0,
            phase: ScrollPhase::Changed,
        });

        assert!(should_handle_surface_event(true, &scroll));
        assert!(!should_handle_surface_event(false, &scroll));
    }

    mod reconciliation {
        use super::*;
        use std::cell::Cell;

        fn surface_with_live_dial(vm: &mut ScriptVm) -> ClassDiagramSurface {
            use waml::model::{ElementType, UmlMetaclass};

            let mut surface = ClassDiagramSurface::script_new(vm);
            let node = |key: &str, title: &str, x: f64| crate::scene::SceneNode {
                key: key.into(),
                title: title.into(),
                element_type: ElementType::Uml(UmlMetaclass::Class),
                stereotypes: Vec::new(),
                stereotype_visible: true,
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
        fn real_touch_events_mutate_only_an_enabled_surface() {
            let mut vm = crate::script_gate::boot_test_vm();
            let mut surface = surface_with_live_dial(&mut vm);
            surface.viewport.set_view_rect(Rect {
                pos: dvec2(0.0, 0.0),
                size: dvec2(800.0, 600.0),
            });
            surface
                .selection
                .select("old-subject", &surface.scene.nodes);
            let camera_before = surface.viewport.snapshot().camera;
            let selected_before = surface.selection.selected_key().map(str::to_owned);

            surface.set_interaction_enabled(vm.cx_mut(), false);
            dispatch_touch_update(
                &mut surface,
                &mut vm,
                vec![
                    touch(1, TouchState::Start, 100.0, 100.0),
                    touch(2, TouchState::Start, 200.0, 100.0),
                ],
            );
            dispatch_touch_update(
                &mut surface,
                &mut vm,
                vec![
                    touch(1, TouchState::Move, 50.0, 100.0),
                    touch(2, TouchState::Move, 250.0, 100.0),
                ],
            );

            assert!(!surface.interaction_enabled);
            assert_eq!(surface.viewport.snapshot().camera, camera_before);
            assert_eq!(
                surface.selection.selected_key().map(str::to_owned),
                selected_before
            );

            surface.set_interaction_enabled(vm.cx_mut(), true);
            dispatch_touch_update(
                &mut surface,
                &mut vm,
                vec![
                    touch(1, TouchState::Start, 100.0, 100.0),
                    touch(2, TouchState::Start, 200.0, 100.0),
                ],
            );
            dispatch_touch_update(
                &mut surface,
                &mut vm,
                vec![
                    touch(1, TouchState::Move, 50.0, 100.0),
                    touch(2, TouchState::Move, 250.0, 100.0),
                ],
            );

            assert_ne!(surface.viewport.snapshot().camera, camera_before);
            assert_eq!(
                surface.selection.selected_key().map(str::to_owned),
                selected_before
            );
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
        fn replacement_resets_surface_state_and_stops_all_clocks() {
            let mut vm = crate::script_gate::boot_test_vm();
            let mut surface = surface_with_live_dial(&mut vm);
            surface
                .selection
                .select("old-subject", &surface.scene.nodes);
            surface
                .selection
                .set_conflict_focus_keys(Some(vec!["old-subject".into(), "old-reference".into()]));

            let replacement = Scene::default();
            let (interaction_effects, viewport_effects) =
                surface.prepare_scene_reconciliation(&replacement, &SceneUpdate::Replace);

            assert_eq!(surface.placement.snapshot(), Default::default());
            let selection = surface.selection.snapshot();
            assert_eq!(selection.selected_key, None);
            assert_eq!(selection.selected_index, None);
            assert_eq!(selection.conflict_focus_keys, None);
            assert_eq!(interaction_effects.dwell_timer, TimerCommand::Stop);
            assert_eq!(interaction_effects.preview_frame, FrameCommand::Stop);
            assert_eq!(viewport_effects.camera_timer, ViewportTimerCommand::Stop);

            let replacement_bounds = WorldRect {
                x: 10.0,
                y: 20.0,
                w: 80.0,
                h: 60.0,
            };
            surface
                .viewport
                .retain_for_scene_update(Some(replacement_bounds));
            surface.viewport.set_view_rect(Rect {
                pos: dvec2(0.0, 0.0),
                size: dvec2(800.0, 600.0),
            });
            assert!(
                surface.viewport.apply_initial_fit(),
                "an empty replacement must leave a pending scene fit"
            );
        }

        #[test]
        fn placement_cancel_ends_the_viewport_pan() {
            let mut vm = crate::script_gate::boot_test_vm();
            let mut surface = surface_with_live_dial(&mut vm);
            surface.viewport.begin_pan(dvec2(10.0, 10.0));

            let effects = surface.cancel_active_placement();

            assert_eq!(effects.dwell_timer, TimerCommand::Stop);
            assert_eq!(effects.preview_frame, FrameCommand::Stop);
            assert!(
                !surface.viewport.pan_to(dvec2(30.0, 30.0)),
                "a cancelled placement must not leave a live viewport pan"
            );
        }

        fn touch(uid: u64, state: TouchState, x: f64, y: f64) -> TouchPoint {
            TouchPoint {
                state,
                abs: dvec2(x, y),
                time: 0.0,
                uid,
                rotation_angle: 0.0,
                force: 0.0,
                radius: dvec2(1.0, 1.0),
                handled: Cell::new(Area::Empty),
                sweep_lock: Cell::new(Area::Empty),
            }
        }

        fn dispatch_touch_update(
            surface: &mut ClassDiagramSurface,
            vm: &mut ScriptVm,
            touches: Vec<TouchPoint>,
        ) {
            let event = Event::TouchUpdate(TouchUpdateEvent {
                time: 0.0,
                window_id: WindowId(0, 0),
                modifiers: KeyModifiers::default(),
                touches,
            });
            surface.handle_event(vm.cx_mut(), &event, &mut Scope::empty());
        }

        #[test]
        fn production_pinch_takeover_cancels_pan_without_promoting_the_survivor() {
            let mut vm = crate::script_gate::boot_test_vm();
            let mut surface = ClassDiagramSurface::script_new(&mut vm);
            surface.viewport.set_view_rect(Rect {
                pos: dvec2(0.0, 0.0),
                size: dvec2(800.0, 600.0),
            });
            surface.viewport.begin_pan(dvec2(100.0, 100.0));

            dispatch_touch_update(
                &mut surface,
                &mut vm,
                vec![
                    touch(1, TouchState::Start, 100.0, 100.0),
                    touch(2, TouchState::Start, 200.0, 100.0),
                ],
            );
            dispatch_touch_update(
                &mut surface,
                &mut vm,
                vec![
                    touch(1, TouchState::Move, 110.0, 100.0),
                    touch(2, TouchState::Stop, 200.0, 100.0),
                ],
            );

            let camera_after_pinch = surface.viewport.camera();
            assert_eq!(surface.viewport.pan_down_abs(), None);
            assert!(
                !surface.viewport.pan_to(dvec2(130.0, 100.0)),
                "the surviving touch must not resume the pre-pinch pan"
            );
            assert_eq!(surface.viewport.camera(), camera_after_pinch);
        }

        fn assert_scene_update_starts_matching_touch_ids_as_a_new_pinch(update: SceneUpdate) {
            let mut vm = crate::script_gate::boot_test_vm();
            let mut surface = ClassDiagramSurface::script_new(&mut vm);
            surface.viewport.set_view_rect(Rect {
                pos: dvec2(0.0, 0.0),
                size: dvec2(800.0, 600.0),
            });
            surface.viewport.apply_pinch_sample(TouchPair {
                a: 1,
                b: 2,
                spread: 100.0,
                midpoint_abs: dvec2(300.0, 200.0),
            });
            let camera_before_reconciliation = surface.viewport.camera();

            match update {
                SceneUpdate::Replace => surface.set_scene(vm.cx_mut(), Scene::default()),
                SceneUpdate::Focus { .. } => surface.set_focus(vm.cx_mut(), Scene::default()),
                SceneUpdate::PreserveViewport => {
                    surface.update_scene(vm.cx_mut(), Scene::default())
                }
            }

            let restarted = surface.viewport.apply_pinch_sample(TouchPair {
                a: 1,
                b: 2,
                spread: 200.0,
                midpoint_abs: dvec2(360.0, 240.0),
            });
            assert_eq!(surface.viewport.camera(), camera_before_reconciliation);
            assert!(!restarted.redraw);
            assert_eq!(restarted.camera_timer, ViewportTimerCommand::Stop);

            let continued = surface.viewport.apply_pinch_sample(TouchPair {
                a: 1,
                b: 2,
                spread: 150.0,
                midpoint_abs: dvec2(380.0, 250.0),
            });
            assert!(continued.redraw);
            assert_ne!(surface.viewport.camera(), camera_before_reconciliation);
        }

        #[test]
        fn every_scene_reconciliation_starts_matching_touch_ids_as_a_new_pinch() {
            assert_scene_update_starts_matching_touch_ids_as_a_new_pinch(SceneUpdate::Replace);
            assert_scene_update_starts_matching_touch_ids_as_a_new_pinch(SceneUpdate::Focus {
                key: "ignored-by-public-set-focus".into(),
            });
            assert_scene_update_starts_matching_touch_ids_as_a_new_pinch(
                SceneUpdate::PreserveViewport,
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
        fn stale_projection_state_is_explicit_and_reversible() {
            let mut vm = crate::script_gate::boot_test_vm();
            let mut surface = ClassDiagramSurface::script_new(&mut vm);

            surface.set_projection_stale(vm.cx_mut(), true);
            assert!(surface.projection_stale());
            surface.set_projection_stale(vm.cx_mut(), false);
            assert!(!surface.projection_stale());
        }

        #[test]
        fn stale_badge_is_fixed_to_the_canvas_top_right() {
            assert_eq!(
                stale_badge_rect(Rect {
                    pos: dvec2(100.0, 50.0),
                    size: dvec2(800.0, 600.0),
                }),
                Rect {
                    pos: dvec2(830.0, 62.0),
                    size: dvec2(58.0, 24.0),
                }
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
