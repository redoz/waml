//! `BehaviorSurface`: the kind-agnostic behavior-canvas widget (spec §1.2-1.3,
//! §5.3). Read-only, like `ClassDiagramSurface`: pan/zoom over a `Camera`,
//! no editing. Task 6 wires only the empty-state render + pan/zoom skeleton
//! -- `Flow`/`Interaction` scenes and their renderers land in Tasks 7-8.

pub(crate) mod hit;
mod render;
pub(crate) mod scene;

use crate::canvas::viewport::{
    TimerCommand as ViewportTimerCommand, ViewportController, ViewportEffects,
};
use hit::BehaviorTarget;
use makepad_widgets::*;
use render::BehaviorDrawResources;
use scene::BehaviorScene;

script_mod! {
    use mod.prelude.widgets_internal.*
    use mod.atlas
    use mod.fonts
    use mod.widgets.*
    use mod.text.*

    mod.widgets.BehaviorSurfaceBase = #(BehaviorSurface::register_widget(vm))

    // Flow node pen: rounded rect (`Plain`, radius pushed to a real value) or
    // sharp rect (`Object`, radius pushed to 0) -- one shader covers both
    // since a box with radius 0 IS a sharp rect (spec §4.1).
    mod.draw.FlowBox = mod.draw.DrawColor{
        radius: uniform(6.0)
        pixel: fn() {
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            sdf.box(0.0, 0.0, self.rect_size.x, self.rect_size.y, self.radius)
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

    // Open message-head pen (`sends`/`replies`, spec §4.2): two open strokes
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

    // Fragment-tab pentagon pen (spec §4.2): the "house" 5-point path carrying
    // the kind keyword (`alt`/`opt`/`loop`), notched on the right edge.
    mod.draw.InteractionPentagon = mod.draw.DrawColor{
        pixel: fn() {
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            let w = self.rect_size.x
            let h = self.rect_size.y
            let notch = min(w * 0.25, h * 0.5)
            sdf.move_to(0.0, 0.0)
            sdf.line_to(w - notch, 0.0)
            sdf.line_to(w, h * 0.5)
            sdf.line_to(w - notch, h)
            sdf.line_to(0.0, h)
            sdf.close_path()
            sdf.fill(self.color)
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
        draw_pentagon: mod.draw.InteractionPentagon{ color: atlas.field_bg }
        draw_fill +: { color: atlas.text_dim }
        draw_text +: {
            color: atlas.text_dim
            text_style: fonts.text_body
        }
        draw_text_heading +: {
            color: atlas.text
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

    #[rust]
    scene: BehaviorScene,
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
                let view_rect = self.viewport.snapshot().view_rect;
                let (wx, wy) = self
                    .viewport
                    .camera()
                    .local_to_world(me.abs.x - view_rect.pos.x, me.abs.y - view_rect.pos.y);
                let target = hit::hit_test(&self.scene, (wx, wy));
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
            accent,
        };
        render::draw(
            cx,
            self.viewport.snapshot(),
            &self.scene,
            self.hovered.as_ref(),
            self.selected.as_ref(),
            &mut draws,
        );
        DrawStep::done()
    }
}

impl BehaviorSurface {
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

    pub(crate) fn set_scene(&mut self, cx: &mut Cx, scene: BehaviorScene) {
        self.scene = scene;
        self.draw_bg.redraw(cx);
    }

    /// The current persistent selection, if any (spec §5.2).
    #[allow(dead_code)]
    pub(crate) fn selected(&self) -> Option<&BehaviorTarget> {
        self.selected.as_ref()
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
}
