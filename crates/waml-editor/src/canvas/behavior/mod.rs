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

    mod.widgets.BehaviorSurface = set_type_default() do mod.widgets.BehaviorSurfaceBase{
        width: Fill
        height: Fill
        draw_bg +: { color: atlas.canvas_ground }
        draw_text +: {
            color: atlas.text_dim
            text_style: fonts.text_body
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

    #[rust]
    scene: BehaviorScene,
    #[rust]
    viewport: ViewportController,
    #[rust]
    cam_timer: Timer,
    #[rust]
    pointer_down_abs: Option<DVec2>,
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
                    let action = match hit::hit_test(&self.scene, (wx, wy)) {
                        Some(target) => BehaviorSurfaceAction::Selected(Some(target)),
                        None => BehaviorSurfaceAction::Cleared,
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
        let message = match &self.scene {
            BehaviorScene::Empty { message } => message.clone(),
        };
        let mut draws = BehaviorDrawResources {
            bg: &mut self.draw_bg,
            text: &mut self.draw_text,
        };
        render::draw(cx, self.viewport.snapshot(), &message, &mut draws);
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
