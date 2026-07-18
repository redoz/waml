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
    use mod.text.*

    mod.widgets.GraphCanvasBase = #(GraphCanvas::register_widget(vm))

    mod.widgets.GraphCanvas = set_type_default() do mod.widgets.GraphCanvasBase{
        width: Fill
        height: Fill
        draw_bg +: { color: #x1b1b24 }
        draw_group +: { color: #x24242f }
        draw_node +: { color: #x3a3a52 }
        draw_edge +: { color: #x6b6b8a }
        draw_text +: {
            color: #xf0f0f6
            text_style: TextStyle{
                font_size: 12
                font_family: FontFamily{
                    latin := FontMember{res: crate_resource("self:resources/fonts/IBMPlexSans-Regular.ttf") asc: -0.1 desc: 0.0}
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
    draw_edge: DrawColor,
    #[redraw]
    #[live]
    draw_text: DrawText,

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

/// Intersection of the center-to-center line from `from` to `to` with `from`'s
/// border, in world coordinates. Operates on `waml::solve::Rect` (`x`/`y`/`w`/`h`),
/// the type `SceneEdge` carries. Used to clip edge endpoints to node borders.
fn border_point(from: waml::solve::Rect, to: waml::solve::Rect) -> (f64, f64) {
    let fcx = from.x + from.w * 0.5;
    let fcy = from.y + from.h * 0.5;
    let tcx = to.x + to.w * 0.5;
    let tcy = to.y + to.h * 0.5;
    let dx = tcx - fcx;
    let dy = tcy - fcy;
    if dx == 0.0 && dy == 0.0 {
        return (fcx, fcy);
    }
    let hw = from.w * 0.5;
    let hh = from.h * 0.5;
    // Scale the direction vector to the nearest border along x and y, take the closer.
    let tx = if dx != 0.0 {
        (hw / dx).abs()
    } else {
        f64::INFINITY
    };
    let ty = if dy != 0.0 {
        (hh / dy).abs()
    } else {
        f64::INFINITY
    };
    let t = tx.min(ty);
    (fcx + dx * t, fcy + dy * t)
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

        if !self.fitted && rect.size.x > 0.0 && rect.size.y > 0.0 {
            if let Some(bbox) = bounding_box(&self.scene) {
                self.camera = if self.focus_mode {
                    let zoom = 1.5;
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
                self.draw_text
                    .draw_abs(cx, dvec2(screen.pos.x + 6.0, screen.pos.y + 4.0), title);
            }
        }

        // Edges: straight segment from source border to target border, drawn as a
        // thin axis-aligned quad. Rotated-quad / arrow styling is a fast-follow.
        for edge in &self.scene.edges {
            let (sx, sy) = border_point(edge.source, edge.target);
            let (tx, ty) = border_point(edge.target, edge.source);
            let (a0, a1) = self.camera.world_to_local(sx, sy);
            let (b0, b1) = self.camera.world_to_local(tx, ty);
            let a = dvec2(rect.pos.x + a0, rect.pos.y + a1);
            let b = dvec2(rect.pos.x + b0, rect.pos.y + b1);
            let len = ((b.x - a.x).powi(2) + (b.y - a.y).powi(2)).sqrt();
            if len < 1e-3 {
                continue;
            }
            let thickness = 2.0;
            // Axis-aligned bounding box of the segment: reads correctly for the
            // orthogonal arrangements typical of `## Layout` diagrams.
            let min = dvec2(a.x.min(b.x), a.y.min(b.y));
            let max = dvec2(a.x.max(b.x), a.y.max(b.y));
            let seg = Rect {
                pos: min,
                size: dvec2(
                    (max.x - min.x).max(thickness),
                    (max.y - min.y).max(thickness),
                ),
            };
            self.draw_edge.draw_abs(cx, seg);
        }

        // Nodes: drawn last so they sit on top of groups and edges.
        for node in &self.scene.nodes {
            let (lx, ly) = self.camera.world_to_local(node.rect.x, node.rect.y);
            let screen = Rect {
                pos: dvec2(rect.pos.x + lx, rect.pos.y + ly),
                size: dvec2(
                    node.rect.w * self.camera.zoom,
                    node.rect.h * self.camera.zoom,
                ),
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
        self.focus_mode = false;
        self.draw_bg.redraw(cx);
    }

    /// Like `set_scene`, but pins the camera at 1.5x zoom centered on the
    /// node instead of fitting the whole scene to the view. Used for the
    /// classifier-focus doc tab.
    pub fn set_focus(&mut self, cx: &mut Cx, scene: Scene) {
        self.scene = scene;
        self.fitted = false;
        self.focus_mode = true;
        self.draw_bg.redraw(cx);
    }

    /// Node count of the current scene, for the statusbar mock.
    pub fn node_count(&self) -> usize {
        self.scene.nodes.len()
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
    fn border_point_exits_on_the_side_facing_the_target() {
        // 100x100 box at origin; target far to the right -> exit on right edge x=100.
        let from = WorldRect {
            x: 0.0,
            y: 0.0,
            w: 100.0,
            h: 100.0,
        };
        let to = WorldRect {
            x: 500.0,
            y: 0.0,
            w: 100.0,
            h: 100.0,
        };
        let (x, y) = border_point(from, to);
        assert!((x - 100.0).abs() < 1e-6, "x = {x}");
        assert!((y - 50.0).abs() < 1e-6, "y = {y}");
    }

    #[test]
    fn border_point_handles_vertical_stack() {
        // Target directly below -> exit on bottom edge y=100, centered x=50.
        let from = WorldRect {
            x: 0.0,
            y: 0.0,
            w: 100.0,
            h: 100.0,
        };
        let to = WorldRect {
            x: 0.0,
            y: 400.0,
            w: 100.0,
            h: 100.0,
        };
        let (x, y) = border_point(from, to);
        assert!((x - 50.0).abs() < 1e-6, "x = {x}");
        assert!((y - 100.0).abs() < 1e-6, "y = {y}");
    }
}
