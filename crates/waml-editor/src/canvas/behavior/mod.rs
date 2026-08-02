//! `BehaviorSurface`: the kind-agnostic behavior-canvas widget (spec §1.2-1.3,
//! §5.3). Read-only, like `ClassDiagramSurface`: pan/zoom over a `Camera`,
//! no editing. Task 6 wires only the empty-state render + pan/zoom skeleton
//! -- `Flow`/`Interaction` scenes and their renderers land in Tasks 7-8.

pub(crate) mod hit;
mod render;
pub(crate) mod scene;

use crate::canvas::viewport::{
    InitialFit, TimerCommand as ViewportTimerCommand, ViewportController, ViewportEffects,
    ViewportSnapshot,
};
use hit::BehaviorTarget;
use makepad_widgets::*;
use render::{BehaviorDrawResources, BehaviorPalette};
use scene::BehaviorScene;

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
    use mod.fonts
    use mod.widgets.*
    use mod.text.*

    mod.widgets.BehaviorSurfaceBase = #(BehaviorSurface::register_widget(vm))

    // Flow node pen: rounded rect (`Plain`, radius pushed to a real value) or
    // sharp rect (`Object`, radius pushed to 0) (spec §4.1). The two need
    // different SDF calls: `sdf.box` DEGENERATES at radius 0 and draws nothing,
    // which is why an object node used to render as bare text with no box.
    mod.draw.FlowBox = mod.draw.DrawColor{
        radius: uniform(6.0)
        pixel: fn() {
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            if self.radius <= 0.0 {
                sdf.rect(0.0, 0.0, self.rect_size.x, self.rect_size.y)
            } else {
                sdf.box(0.0, 0.0, self.rect_size.x, self.rect_size.y, self.radius)
            }
            sdf.fill(self.color)
            return sdf.result
        }
    }

    // `Decision`/`Merge` diamond: an explicit 4-point SDF path, never a
    // zero-radius box (spec §4.1).
    mod.draw.FlowDiamond = mod.draw.DrawColor{
        pixel: fn() {
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            let w = self.rect_size.x
            let h = self.rect_size.y
            sdf.move_to(w * 0.5, 0.0)
            sdf.line_to(w, h * 0.5)
            sdf.line_to(w * 0.5, h)
            sdf.line_to(0.0, h * 0.5)
            sdf.close_path()
            sdf.fill(self.color)
            return sdf.result
        }
    }

    // `Initial`/`Final` disc pen: `bullseye` 0 -> a single filled circle;
    // `bullseye` 1 -> outer disc minus a 72%-radius annulus hole (the same
    // circle-subtract-circle technique `EdgeElbow`'s fillet band uses) UNION
    // a 40%-radius center dot, giving the ring-plus-dot glyph in one draw
    // rather than three separately-composited instances of this pen (which
    // do not visually layer -- each `draw_abs` call is its own instanced
    // quad, not a persistent framebuffer the next call paints over).
    mod.draw.FlowCircle = mod.draw.DrawColor{
        bullseye: uniform(0.0)
        pixel: fn() {
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            let cx_ = self.rect_size.x * 0.5
            let cy_ = self.rect_size.y * 0.5
            let r = min(self.rect_size.x, self.rect_size.y) * 0.5
            sdf.circle(cx_, cy_, r)
            sdf.circle(cx_, cy_, r * 0.72 * self.bullseye)
            sdf.subtract()
            sdf.circle(cx_, cy_, r * 0.4 * self.bullseye)
            sdf.fill(self.color)
            return sdf.result
        }
    }

    // Route arrowhead pen: an explicit 3-point SDF path in quad-local pixels
    // (mirrors `EdgeMarker`'s `v01`/`v23` convention, one vertex pair short).
    mod.draw.FlowTriangle = mod.draw.DrawColor{
        v0: uniform(vec2(0.0, 0.0))
        v1: uniform(vec2(0.0, 0.0))
        v2: uniform(vec2(0.0, 0.0))
        pixel: fn() {
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            sdf.move_to(self.v0.x, self.v0.y)
            sdf.line_to(self.v1.x, self.v1.y)
            sdf.line_to(self.v2.x, self.v2.y)
            sdf.close_path()
            sdf.fill(self.color)
            return sdf.result
        }
    }

// Open message-head pen (`signals`/`returns`, spec §4.2): two open strokes
    // from the apex `v0` out to wing tips `v1`/`v2` -- no closing base, no
    // fill, so it reads as a caret rather than `FlowTriangle`'s solid head.
    mod.draw.InteractionOpenHead = mod.draw.DrawColor{
        v0: uniform(vec2(0.0, 0.0))
        v1: uniform(vec2(0.0, 0.0))
        v2: uniform(vec2(0.0, 0.0))
        stroke_w: uniform(1.4)
        pixel: fn() {
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            sdf.move_to(self.v1.x, self.v1.y)
            sdf.line_to(self.v0.x, self.v0.y)
            sdf.line_to(self.v2.x, self.v2.y)
            sdf.stroke(self.color, self.stroke_w)
            return sdf.result
        }
    }

    // Destroyed-lifeline X pen (spec §4.2): two crossing SDF strokes, built as
    // ONE draw (per the Task 7 handoff gotcha -- layering separate draw_abs
    // calls on one shader does not visually composite).
    mod.draw.InteractionXMark = mod.draw.DrawColor{
        stroke_w: uniform(1.6)
        pixel: fn() {
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            let w = self.rect_size.x
            let h = self.rect_size.y
            sdf.move_to(0.0, 0.0)
            sdf.line_to(w, h)
            sdf.stroke(self.color, self.stroke_w)
            sdf.move_to(w, 0.0)
            sdf.line_to(0.0, h)
            sdf.stroke(self.color, self.stroke_w)
            return sdf.result
        }
    }

    // Combined-fragment frame border (spec §4.2): a stroked outline, no fill,
    // so nested frames and messages inside remain visible.
    mod.draw.InteractionFrameBorder = mod.draw.DrawColor{
        stroke_w: uniform(1.2)
        pixel: fn() {
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            let inset = self.stroke_w * 0.5
            sdf.rect(inset, inset, self.rect_size.x - inset * 2.0, self.rect_size.y - inset * 2.0)
            sdf.stroke(self.color, self.stroke_w)
            return sdf.result
        }
    }

    // Fragment-tab pen (spec §4.2): the kind keyword's plate in the frame's
    // top-left corner. UML cuts the BOTTOM-RIGHT corner off a rectangle -- the
    // tab is a rectangle with one bevel, not an arrow. The earlier "house"
    // path (a full-height point on the right edge) both read as an arrow and
    // ate the keyword, because the point is where the last glyphs land.
    //
    // The bevel is a fixed fraction of the plate height, so it stays a corner
    // cut at every label width instead of growing into the text.
    mod.draw.InteractionTab = mod.draw.DrawColor{
        border_col: uniform(#x00000000)
        stroke_w: uniform(1.0)
        pixel: fn() {
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            let inset = self.stroke_w * 0.5
            let w = self.rect_size.x - inset
            let h = self.rect_size.y - inset
            let notch = min(h * 0.45, w * 0.5)
            sdf.move_to(inset, inset)
            sdf.line_to(w, inset)
            sdf.line_to(w, h - notch)
            sdf.line_to(w - notch, h)
            sdf.line_to(inset, h)
            sdf.close_path()
            sdf.fill_keep(self.color)
            sdf.stroke(self.border_col, self.stroke_w)
            return sdf.result
        }
    }

    mod.widgets.BehaviorSurface = set_type_default() do mod.widgets.BehaviorSurfaceBase{
        width: Fill
        height: Fill
        draw_bg +: { color: atlas.canvas_ground }
        draw_node_box: mod.draw.FlowBox{ color: atlas.field_bg }
        draw_diamond: mod.draw.FlowDiamond{ color: atlas.field_bg }
        draw_circle: mod.draw.FlowCircle{ color: atlas.text }
        draw_triangle: mod.draw.FlowTriangle{ color: atlas.text_dim }
        draw_open_head: mod.draw.InteractionOpenHead{ color: atlas.text_dim }
        draw_x_mark: mod.draw.InteractionXMark{ color: atlas.text_dim }
        draw_frame_border: mod.draw.InteractionFrameBorder{ color: atlas.text_dim }
        draw_pentagon: mod.draw.InteractionTab{ color: atlas.field_bg }
        // The lifeline head is a real Atlas surface: the app-wide 150deg
        // gradient border, re-tinted per lifeline to that participant's accent
        // (`draw_head`), over a wash of the same accent. Frost is off -- the
        // head carries the accent wash the canvas has always drawn, not the
        // near-white card fill.
        draw_head: mod.draw.AccentFrame{
            frost_top: 1.0 frost_bot: 1.0
            depth_y: 0.0 depth_blur: 0.0 depth_a: 0.0 bloom: 0.0
        }
        draw_fill +: { color: atlas.text_dim }
        // Route/stem/frame resting stroke plus the two call-out colours, as
        // atlas roles: hover darkens to full-strength text, selection takes
        // the theme's interaction accent (the same language the rest of the
        // chrome speaks), so both track light/dark.
        line_color: atlas.bucket_slate
        hover_color: atlas.text
        select_color: atlas.accent
        draw_text +: {
            color: atlas.text_dim
            text_style: fonts.text_body
        }
        draw_text_heading +: {
            color: atlas.text
            text_style: fonts.text_heading
        }
        draw_stale_badge +: { color: atlas.bucket_amber }
        draw_stale_text +: {
            color: atlas.canvas_ground
            text_style: fonts.text_heading
        }
    }
}

#[derive(Script, ScriptHook, Widget)]
pub struct BehaviorSurface {
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
    draw_text: DrawText,
    #[redraw]
    #[live]
    draw_text_heading: DrawText,
    #[redraw]
    #[live]
    draw_node_box: DrawColor,
    #[redraw]
    #[live]
    draw_diamond: DrawColor,
    #[redraw]
    #[live]
    draw_circle: DrawColor,
    #[redraw]
    #[live]
    draw_triangle: DrawColor,
    #[redraw]
    #[live]
    draw_fill: DrawColor,
    #[redraw]
    #[live]
    draw_open_head: DrawColor,
    #[redraw]
    #[live]
    draw_x_mark: DrawColor,
    #[redraw]
    #[live]
    draw_frame_border: DrawColor,
    #[redraw]
    #[live]
    draw_pentagon: DrawColor,
    #[redraw]
    #[live]
    draw_head: DrawColor,
    #[redraw]
    #[live]
    draw_stale_badge: DrawColor,
    #[redraw]
    #[live]
    draw_stale_text: DrawText,

    /// Theme roles the renderers stroke with (`render::BehaviorPalette`).
    #[live]
    line_color: Vec4,
    #[live]
    hover_color: Vec4,
    #[live]
    select_color: Vec4,

    #[rust]
    scene: BehaviorScene,
    /// Bounds of the scene the load-time fit was last requested for, so an
    /// unchanged re-`sync` does not stomp the user's settled camera.
    #[rust]
    fitted_bounds: Option<waml::solve::Rect>,
    #[rust]
    viewport: ViewportController,
    #[rust]
    cam_timer: Timer,
    #[rust]
    pointer_down_abs: Option<DVec2>,
    #[rust]
    hovered: Option<BehaviorTarget>,
    /// The persistently-selected target (spec §5.2). `Esc`/`clear_selection`
    /// clears it; a click replaces it; a double-click leaves it untouched and
    /// instead requests the View Source path.
    #[rust]
    selected: Option<BehaviorTarget>,
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

/// The click-vs-drag threshold, in canvas-local pixels (mirrors
/// `class::SELECT_SLOP`).
const CLICK_SLOP: f64 = 4.0;

/// The behavior element under the raw absolute pointer position `abs`, or
/// `None` when `abs` is outside the canvas rect entirely -- the pointer is over
/// the inspector dock, tree panel, or caption, none of which may tint this
/// canvas (the containment check the raw-`MouseMove` idiom requires).
fn hover_target_at(
    scene: &BehaviorScene,
    viewport: ViewportSnapshot,
    abs: DVec2,
) -> Option<BehaviorTarget> {
    let view_rect = viewport.view_rect;
    if !view_rect.contains(abs) {
        return None;
    }
    let (wx, wy) = viewport
        .camera
        .local_to_world(abs.x - view_rect.pos.x, abs.y - view_rect.pos.y);
    hit::hit_test(scene, (wx, wy))
}

/// Canvas -> App action (same convention as `ClassDiagramSurfaceAction`).
#[derive(Clone, Debug, Default, PartialEq)]
pub enum BehaviorSurfaceAction {
    #[default]
    None,
    Selected(Option<BehaviorTarget>),
    Cleared,
    /// A double-click landed on `BehaviorTarget` (spec §5.2 View Source, Task
    /// 9): the surface has no context menu, so this is the read-only
    /// equivalent affordance that reaches the same navigation path.
    ViewSourceRequested(BehaviorTarget),
}

impl Widget for BehaviorSurface {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, _scope: &mut Scope) {
        if !should_handle_surface_event(self.interaction_enabled, event) {
            return;
        }
        if let Some(te) = self.cam_timer.is_event(event) {
            let effects = self.viewport.tick_camera(te.time.unwrap_or(0.0));
            self.apply_viewport_effects(cx, effects);
        }
        // Hover tracks the raw `MouseMove`, not `Hit::FingerHoverIn` -- this
        // widget has no children now, but a containment check off the raw
        // event is the same idiom the scrim hover fix settled on, and it
        // costs nothing here (mirrors the aligned-parent hit-rect trap: the
        // event carries absolute coords, so translate before hit-testing).
        if let Event::MouseMove(me) = event {
            if self.pointer_down_abs.is_none() {
                let target = hover_target_at(&self.scene, self.viewport.snapshot(), me.abs);
                if target != self.hovered {
                    self.hovered = target;
                    self.draw_bg.redraw(cx);
                }
            }
        }
        match event.hits_with_capture_overload(cx, self.draw_bg.area(), false) {
            Hit::FingerDown(fe) if fe.is_primary_hit() => {
                let effects = self.viewport.cancel_glide();
                self.apply_viewport_effects(cx, effects);
                self.pointer_down_abs = Some(fe.abs);
                self.viewport.begin_pan(fe.abs);
                cx.set_cursor(MouseCursor::Grabbing);
            }
            Hit::FingerMove(fe) => {
                if self.viewport.pan_to(fe.abs) {
                    self.draw_bg.redraw(cx);
                }
            }
            Hit::FingerUp(fe) if fe.is_primary_hit() => {
                self.viewport.end_pan();
                let clicked = self
                    .pointer_down_abs
                    .map(|down| down.distance(&fe.abs) <= CLICK_SLOP)
                    .unwrap_or(false);
                self.pointer_down_abs = None;
                if clicked {
                    let (wx, wy) = self.viewport.camera().local_to_world(
                        fe.abs.x - self.viewport.snapshot().view_rect.pos.x,
                        fe.abs.y - self.viewport.snapshot().view_rect.pos.y,
                    );
                    let target = hit::hit_test(&self.scene, (wx, wy));
                    let action = match (fe.tap_count >= 2, target) {
                        (true, Some(target)) => BehaviorSurfaceAction::ViewSourceRequested(target),
                        (_, Some(target)) => {
                            self.selected = Some(target.clone());
                            BehaviorSurfaceAction::Selected(Some(target))
                        }
                        (_, None) => {
                            self.selected = None;
                            BehaviorSurfaceAction::Cleared
                        }
                    };
                    cx.widget_action(self.widget_uid(), action);
                }
                cx.set_cursor(MouseCursor::Grab);
            }
            Hit::FingerUp(_fe) => {
                self.viewport.end_pan();
                self.pointer_down_abs = None;
                cx.set_cursor(MouseCursor::Grab);
            }
            Hit::FingerHoverIn(_) => cx.set_cursor(MouseCursor::Grab),
            Hit::FingerScroll(fs) => {
                let scroll = if fs.scroll.y.abs() > f64::EPSILON {
                    fs.scroll.y
                } else {
                    fs.scroll.x
                };
                let factor = (-scroll / 240.0).exp2();
                let effects = self.viewport.apply_scroll_zoom(fs.abs, factor);
                self.apply_viewport_effects(cx, effects);
            }
            _ => {}
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, _scope: &mut Scope, walk: Walk) -> DrawStep {
        let rect = cx.walk_turtle(walk);
        self.viewport.set_view_rect(rect);
        self.viewport.apply_initial_fit();
        let accent = crate::accent::tree_kind_color(crate::tree::TreeKind::Behavior)
            .unwrap_or(self.draw_text.color);
        let palette = BehaviorPalette {
            line: self.line_color,
            hovered: self.hover_color,
            selected: self.select_color,
        };
        let mut draws = BehaviorDrawResources {
            bg: &mut self.draw_bg,
            text: &mut self.draw_text,
            node_box: &mut self.draw_node_box,
            diamond: &mut self.draw_diamond,
            circle: &mut self.draw_circle,
            triangle: &mut self.draw_triangle,
            fill: &mut self.draw_fill,
            text_heading: &mut self.draw_text_heading,
            open_head: &mut self.draw_open_head,
            x_mark: &mut self.draw_x_mark,
            frame_border: &mut self.draw_frame_border,
            pentagon: &mut self.draw_pentagon,
            head: &mut self.draw_head,
            accent,
            palette,
        };
        render::draw(
            cx,
            self.viewport.snapshot(),
            &self.scene,
            self.hovered.as_ref(),
            self.selected.as_ref(),
            &mut draws,
        );
        if self.projection_stale {
            draw_stale_badge_overlay(
                cx,
                rect,
                &mut self.draw_stale_badge,
                &mut self.draw_stale_text,
            );
        }
        DrawStep::done()
    }
}

impl BehaviorSurface {
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

    /// Enable or disable every raw camera interaction. Disabling cancels any
    /// in-flight glide without changing the settled camera (mirrors
    /// `ClassDiagramSurface::set_interaction_enabled`).
    pub(crate) fn set_interaction_enabled(&mut self, cx: &mut Cx, enabled: bool) {
        if self.interaction_enabled == enabled {
            return;
        }
        self.interaction_enabled = enabled;
        if !enabled {
            let effects = self.viewport.cancel_glide();
            self.apply_viewport_effects(cx, effects);
            self.viewport.end_pan();
            self.pointer_down_abs = None;
        }
    }

    /// Install `scene` and, when its bounds differ from the scene this surface
    /// last fitted (a first load or a switch to another behavior document),
    /// request a load-time fit the next `draw_walk` applies through
    /// `Camera::fit`/`FIT_PAD` (spec §4, Task 6). An identical re-`sync` (the
    /// per-revision case) leaves the settled camera alone.
    #[cfg(test)]
    pub(crate) fn interaction_enabled(&self) -> bool {
        self.interaction_enabled
    }

    pub(crate) fn set_scene(&mut self, cx: &mut Cx, scene: BehaviorScene) {
        let bounds = scene.bounds();
        if bounds != self.fitted_bounds {
            self.fitted_bounds = bounds;
            self.viewport.request_initial_fit(match bounds {
                Some(bounds) => InitialFit::Scene(bounds),
                None => InitialFit::ScenePending,
            });
        }
        self.install_scene(scene);
        self.draw_bg.redraw(cx);
    }

    /// Install an affected same-document solve without changing the settled
    /// viewport. Keep the selected/hovered target only while its identity is
    /// still present in the replacement scene.
    pub(crate) fn update_scene(&mut self, cx: &mut Cx, scene: BehaviorScene) {
        self.install_scene(scene);
        self.draw_bg.redraw(cx);
    }

    fn install_scene(&mut self, scene: BehaviorScene) {
        if self
            .selected
            .as_ref()
            .is_some_and(|target| hit::target_bounds(&scene, target).is_none())
        {
            self.selected = None;
        }
        if self
            .hovered
            .as_ref()
            .is_some_and(|target| hit::target_bounds(&scene, target).is_none())
        {
            self.hovered = None;
        }
        self.scene = scene;
    }

    /// Frame the whole scene (the view bar's Fit to Size). An `Empty` scene or a
    /// not-yet-drawn canvas is a no-op (mirrors
    /// `ClassDiagramSurface::fit_to_scene`).
    pub(crate) fn fit_to_scene(&mut self, cx: &mut Cx) {
        let effects = self.viewport.fit_to_bounds(self.scene.bounds());
        self.apply_viewport_effects(cx, effects);
    }

    /// Frame the selected element (the view bar's Fit to Selection). No
    /// selection, or a selection with no geometry in this scene, is a no-op.
    pub(crate) fn fit_to_selection(&mut self, cx: &mut Cx) {
        let Some(bounds) = self
            .selected
            .as_ref()
            .and_then(|target| hit::target_bounds(&self.scene, target))
        else {
            return;
        };
        let effects = self.viewport.fit_to_bounds(Some(bounds));
        self.apply_viewport_effects(cx, effects);
    }

    /// Whether an element is currently selected -- drives the view bar's
    /// fit-to-selection button between enabled and dim.
    pub(crate) fn has_selection(&self) -> bool {
        self.selected.is_some()
    }

    pub(crate) fn selected_target(&self) -> Option<&BehaviorTarget> {
        self.selected.as_ref()
    }

    /// Element count of the current scene, for the status bar (mirrors
    /// `ClassDiagramSurface::node_count`).
    pub(crate) fn node_count(&self) -> usize {
        self.scene.element_count()
    }

    /// Current zoom as a whole-number percentage, for the status bar (mirrors
    /// `ClassDiagramSurface::zoom_pct`).
    pub(crate) fn zoom_pct(&self) -> i32 {
        (self.viewport.camera().zoom * 100.0).round() as i32
    }

    /// Drive the selection from outside the canvas -- the inspector's element
    /// picker, which selects the participant the reader just picked. `None`
    /// deselects (picking the document row is deselecting). Selection-only: the
    /// camera is left where the reader put it.
    pub(crate) fn select_target(&mut self, cx: &mut Cx, target: Option<BehaviorTarget>) {
        if self.selected != target {
            self.selected = target;
            self.draw_bg.redraw(cx);
        }
    }

    /// `Esc` clears the selection (spec §5.2).
    pub(crate) fn clear_selection(&mut self, cx: &mut Cx) {
        if self.selected.is_some() {
            self.selected = None;
            self.draw_bg.redraw(cx);
        }
    }

    /// Zoom by `factor` about the viewport centre (mirrors
    /// `ClassDiagramSurface::zoom_step`).
    pub(crate) fn zoom_step(&mut self, cx: &mut Cx, factor: f64) {
        let effects = self.viewport.zoom_step(factor);
        self.apply_viewport_effects(cx, effects);
    }

    /// Convenience reader for `BehaviorDocView` (mirrors
    /// `ClassDiagramSurface::surface_action`).
    pub(crate) fn surface_action(&self, actions: &Actions) -> Option<BehaviorSurfaceAction> {
        let item = actions.find_widget_action(self.widget_uid())?;
        match item.cast() {
            BehaviorSurfaceAction::None => None,
            action => Some(action),
        }
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use makepad_widgets::event::{ScrollEvent, ScrollPhase};
    use std::cell::Cell;

    #[test]
    fn hidden_surface_rejects_real_raw_scroll_input() {
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

    /// One node big enough to extend PAST the canvas rect in world space, so a
    /// pointer outside the canvas still maps inside it.
    fn flow_scene() -> BehaviorScene {
        BehaviorScene::Flow {
            nodes: vec![scene::FlowNodeGeo {
                key: "n1".into(),
                kind: waml::model::FlowNodeKind::Plain,
                rect: waml::solve::Rect {
                    x: -500.0,
                    y: -500.0,
                    w: 1000.0,
                    h: 1000.0,
                },
                title: "n1".into(),
                lines: Vec::new(),
                type_name: None,
                refines: false,
            }],
            edges: Vec::new(),
            off_page: Vec::new(),
            groups: Vec::new(),
        }
    }

    /// Hover tracks the RAW `MouseMove`, so it must reject a position outside
    /// the canvas rect -- otherwise the cursor over the inspector dock or tree
    /// panel still tints behavior elements.
    #[test]
    fn hover_ignores_positions_outside_the_canvas_rect() {
        let scene = flow_scene();
        let snapshot = ViewportSnapshot {
            camera: crate::canvas::viewport::Camera::default(),
            view_rect: Rect {
                pos: dvec2(200.0, 100.0),
                size: dvec2(400.0, 300.0),
            },
        };
        // Inside the canvas, over the node.
        assert_eq!(
            hover_target_at(&scene, snapshot, dvec2(250.0, 120.0)),
            Some(BehaviorTarget::FlowNode("n1".into()))
        );
        // Up and left of the canvas (the tree panel / caption): the world point
        // it maps to is still INSIDE the node, so only containment can reject
        // it.
        assert_eq!(hover_target_at(&scene, snapshot, dvec2(100.0, 50.0)), None);
    }

    /// A newly-installed scene must be FRAMED, not left at pan(0,0)/zoom 1; an
    /// unchanged re-`sync` must leave the settled camera alone.
    #[test]
    fn a_new_scene_requests_a_load_time_fit_and_a_resync_does_not() {
        use makepad_widgets::ScriptNew;
        let mut vm = crate::script_gate::boot_test_vm();
        let mut surface = BehaviorSurface::script_new(&mut vm);
        let cx = vm.cx_mut();
        surface.viewport.set_view_rect(Rect {
            pos: dvec2(0.0, 0.0),
            size: dvec2(800.0, 600.0),
        });

        surface.set_scene(cx, flow_scene());
        assert!(
            surface.viewport.apply_initial_fit(),
            "a new scene must request a load-time fit"
        );
        let fitted = surface.viewport.camera();
        assert_ne!(fitted, crate::canvas::viewport::Camera::default());

        surface.set_scene(cx, flow_scene());
        assert!(
            !surface.viewport.apply_initial_fit(),
            "an unchanged re-sync must not refit"
        );
        assert_eq!(surface.viewport.camera(), fitted);
    }

    #[test]
    fn affected_scene_update_preserves_camera_and_surviving_selection() {
        use makepad_widgets::ScriptNew;
        let mut vm = crate::script_gate::boot_test_vm();
        let mut surface = BehaviorSurface::script_new(&mut vm);
        surface
            .viewport
            .restore_camera(crate::canvas::viewport::Camera {
                pan_x: 17.0,
                pan_y: 29.0,
                zoom: 1.4,
            });
        surface.selected = Some(BehaviorTarget::FlowNode("n1".into()));
        let camera = surface.viewport.camera();

        surface.update_scene(vm.cx_mut(), flow_scene());

        assert_eq!(surface.viewport.camera(), camera);
        assert_eq!(
            surface.selected,
            Some(BehaviorTarget::FlowNode("n1".into()))
        );

        surface.update_scene(
            vm.cx_mut(),
            BehaviorScene::Empty {
                message: String::new(),
            },
        );
        assert_eq!(surface.viewport.camera(), camera);
        assert_eq!(surface.selected, None);
    }

    #[test]
    fn stale_projection_state_is_explicit_and_reversible() {
        use makepad_widgets::ScriptNew;
        let mut vm = crate::script_gate::boot_test_vm();
        let mut surface = BehaviorSurface::script_new(&mut vm);

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
}
