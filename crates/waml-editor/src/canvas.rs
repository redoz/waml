//! The `GraphCanvas` widget: draws the flattened `Scene` under a pan/zoom
//! `Camera`. Read-only — no editing, no hit-testing of individual nodes.
//! Fits the scene to the view on first draw; left-drag pans; scroll zooms
//! toward the cursor. Each node is a filled rect + its title text.
//!
//! Structure/hit-handling mirror the fork's `widgets/src/map/view.rs`.

use crate::camera::Camera;
use crate::scene::{bounding_box, Scene};
use makepad_widgets::*;

script_mod! {
    use mod.prelude.widgets_internal.*
    use mod.widgets.*

    mod.widgets.GraphCanvasBase = #(GraphCanvas::register_widget(vm))

    mod.widgets.GraphCanvas = set_type_default() do mod.widgets.GraphCanvasBase{
        width: Fill
        height: Fill
        draw_bg +: { color: #x14161d }
        draw_node +: { color: #x2b3345 }
        draw_text +: {
            color: #xe6ebf5
            text_style: theme.font_regular{font_size: 11}
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
    draw_text: DrawText,

    #[rust]
    scene: Scene,
    #[rust]
    camera: Camera,
    #[rust]
    fitted: bool,
    #[rust]
    view_rect: Rect,
    #[rust]
    drag_start_abs: Option<DVec2>,
    #[rust]
    drag_start_pan: (f64, f64),
}

impl Default for Camera {
    fn default() -> Self {
        Camera { pan_x: 0.0, pan_y: 0.0, zoom: 1.0 }
    }
}

// An empty scene is the sensible startup default (fed a real one via set_scene).
impl Default for Scene {
    fn default() -> Self {
        Scene { nodes: vec![], groups: vec![], edges: vec![] }
    }
}

impl Widget for GraphCanvas {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, _scope: &mut Scope) {
        match event.hits_with_capture_overload(cx, self.draw_bg.area(), true) {
            Hit::FingerDown(fe) if fe.is_primary_hit() => {
                self.drag_start_abs = Some(fe.abs);
                self.drag_start_pan = (self.camera.pan_x, self.camera.pan_y);
                cx.set_cursor(MouseCursor::Grabbing);
            }
            Hit::FingerMove(fe) => {
                if let Some(start) = self.drag_start_abs {
                    let delta = fe.abs - start;
                    self.camera.pan_x = self.drag_start_pan.0 - delta.x / self.camera.zoom;
                    self.camera.pan_y = self.drag_start_pan.1 - delta.y / self.camera.zoom;
                    self.draw_bg.redraw(cx);
                }
            }
            Hit::FingerUp(_) => {
                self.drag_start_abs = None;
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

        if !self.fitted {
            if let Some(bbox) = bounding_box(&self.scene) {
                self.camera = Camera::fit(bbox, rect.size.x, rect.size.y, 48.0);
                self.fitted = true;
            }
        }

        // Nodes (groups + edges added in Task 7).
        for node in &self.scene.nodes {
            let (lx, ly) = self.camera.world_to_local(node.rect.x, node.rect.y);
            let screen = Rect {
                pos: dvec2(rect.pos.x + lx, rect.pos.y + ly),
                size: dvec2(node.rect.w * self.camera.zoom, node.rect.h * self.camera.zoom),
            };
            self.draw_node.draw_abs(cx, screen);
            self.draw_text.draw_abs(
                cx,
                dvec2(screen.pos.x + 10.0, screen.pos.y + 10.0),
                &node.title,
            );
        }

        DrawStep::done()
    }
}

impl GraphCanvas {
    pub fn set_scene(&mut self, cx: &mut Cx, scene: Scene) {
        self.scene = scene;
        self.fitted = false;
        self.draw_bg.redraw(cx);
    }
}
