//! The `GraphCanvas` widget: draws the flattened `Scene` under a pan/zoom
//! `Camera`. Read-only — no editing, no hit-testing of individual nodes.
//! Fits the scene to the view on first draw; left-drag pans; scroll zooms
//! toward the cursor. Each node is a filled rect + its title text.
//!
//! Structure/hit-handling mirror the fork's `widgets/src/map/view.rs`.

use crate::camera::Camera;
use crate::node_style::{accent_bucket, stereotype_label, AccentBucket};
use crate::scene::{bounding_box, Scene};
use makepad_widgets::*;

script_mod! {
    use mod.prelude.widgets_internal.*
    use mod.atlas
    use mod.widgets.*
    use mod.text.*

    mod.widgets.GraphCanvasBase = #(GraphCanvas::register_widget(vm))

    mod.widgets.GraphCanvas = set_type_default() do mod.widgets.GraphCanvasBase{
        width: Fill
        height: Fill
        draw_bg +: { color: atlas.canvas_ground }
        draw_group +: { color: atlas.group_fill }
        // Node card: a near-white glass panel carrying the Atlas
        // "source-bright" frame -- the reusable `HudFrame` primitive (see
        // `draw_hud.rs`): a thin accent stroke fading along a 150deg diagonal,
        // bright top-left (`frame_hi`) to dim bottom-right (`frame_lo`). Only
        // the fill differs from the frame defaults, so we override just `color`.
        draw_node: mod.draw.HudFrame{ color: atlas.field_bg }
        draw_edge +: { color: atlas.text_dim }
        // U9 node-kind accent bars (see `node_style::AccentBucket`): a thin
        // strip drawn along a node's top edge, distinct per kind bucket.
        // Colors are the Atlas bucket set (hud-icons-mock.html swatches),
        // assigned in `AccentBucket` declaration order.
        draw_accent_interface +: { color: atlas.bucket_blue }
        draw_accent_enum +: { color: atlas.bucket_cyan }
        draw_accent_note +: { color: atlas.bucket_teal }
        draw_accent_actor +: { color: atlas.bucket_indigo }
        draw_accent_usecase +: { color: atlas.bucket_amber }
        draw_accent_package +: { color: atlas.bucket_green }
        draw_accent_behavior +: { color: atlas.bucket_rose }
        draw_accent_unknown +: { color: atlas.bucket_slate }
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
    draw_edge: DrawColor,
    #[redraw]
    #[live]
    draw_accent_interface: DrawColor,
    #[redraw]
    #[live]
    draw_accent_enum: DrawColor,
    #[redraw]
    #[live]
    draw_accent_note: DrawColor,
    #[redraw]
    #[live]
    draw_accent_actor: DrawColor,
    #[redraw]
    #[live]
    draw_accent_usecase: DrawColor,
    #[redraw]
    #[live]
    draw_accent_package: DrawColor,
    #[redraw]
    #[live]
    draw_accent_behavior: DrawColor,
    #[redraw]
    #[live]
    draw_accent_unknown: DrawColor,
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
            let thickness = 2.0 * zoom;
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

        // Nodes: drawn last so they sit on top of groups and edges. Cloned out
        // of `self.scene` so the body render can take `&mut self`
        // (`draw_focus_card`) without holding an immutable borrow of the scene.
        let nodes = self.scene.nodes.clone();
        for node in &nodes {
            let (lx, ly) = self.camera.world_to_local(node.rect.x, node.rect.y);
            let screen = Rect {
                pos: dvec2(rect.pos.x + lx, rect.pos.y + ly),
                size: dvec2(
                    node.rect.w * self.camera.zoom,
                    node.rect.h * self.camera.zoom,
                ),
            };
            // Node card: rounded near-white glass fill + source-bright accent
            // frame, both in draw_node's SDF shader (see script_mod above).
            self.draw_node.draw_abs(cx, screen);

            // U9: a thin accent bar along the node's top edge, colored by its
            // element-type bucket (`node_style::accent_bucket`). `None` (plain
            // `Class`, `Association`, unresolved `Diagram`) draws nothing --
            // that's the pre-U9 look.
            let bar_h = (4.0 * zoom).min(screen.size.y);
            let bar = Rect {
                pos: screen.pos,
                size: dvec2(screen.size.x, bar_h),
            };
            match accent_bucket(&node.element_type) {
                AccentBucket::None => {}
                AccentBucket::Interface => self.draw_accent_interface.draw_abs(cx, bar),
                AccentBucket::Enum => self.draw_accent_enum.draw_abs(cx, bar),
                AccentBucket::Note => self.draw_accent_note.draw_abs(cx, bar),
                AccentBucket::Actor => self.draw_accent_actor.draw_abs(cx, bar),
                AccentBucket::UseCase => self.draw_accent_usecase.draw_abs(cx, bar),
                AccentBucket::Package => self.draw_accent_package.draw_abs(cx, bar),
                AccentBucket::Behavior => self.draw_accent_behavior.draw_abs(cx, bar),
                AccentBucket::Unknown => self.draw_accent_unknown.draw_abs(cx, bar),
            }

            if self.focus_mode {
                self.draw_focus_card(cx, screen, node, zoom);
            } else {
                // Overview: the compact pre-U9 look -- an optional «stereotype»
                // guillemet line above the title, no compartment body.
                self.draw_text.text_style.font_size = (12.0 * zoom) as f32;
                let mut text_y = screen.pos.y + 10.0 * zoom;
                if let Some(label) = stereotype_label(&node.element_type) {
                    self.draw_text.draw_abs(
                        cx,
                        dvec2(screen.pos.x + 10.0 * zoom, text_y),
                        &format!("\u{ab}{label}\u{bb}"),
                    );
                    text_y += 14.0 * zoom;
                }
                self.draw_text
                    .draw_abs(cx, dvec2(screen.pos.x + 10.0 * zoom, text_y), &node.title);
            }
        }

        DrawStep::done()
    }
}

impl GraphCanvas {
    /// Draw the classifier focus card by laying out its `Shape` box-tree
    /// (`card::class_shape` under `card::mono_sheet`) with taffy and walking the
    /// placed text leaves, each drawn with the mono pen selected by its
    /// (weight, Atlas color) — the card is styled entirely by the box-tree.
    fn draw_focus_card(
        &mut self,
        cx: &mut Cx2d,
        screen: Rect,
        node: &crate::scene::SceneNode,
        zoom: f64,
    ) {
        use crate::card::{self, Token, Weight};
        let texts = card::card_texts(node, &card::mono_sheet());
        for pt in &texts {
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
    }

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
