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
    use mod.atlas
    use mod.widgets.*
    use mod.text.*

    mod.widgets.GraphCanvasBase = #(GraphCanvas::register_widget(vm))

    // Edge pen: stroke the segment as an actual line, NOT a filled rect. The
    // segment's axis-aligned bounding box has the two endpoints at opposite
    // corners, so the edge is one of the AABB's two diagonals; `flip` selects
    // which (0 = top-left->bottom-right, 1 = top-right->bottom-left). Filling
    // the whole AABB (the old `draw_edge: DrawColor`) painted a solid grey
    // rectangle for every diagonal edge -- fine for the orthogonal `## Layout`
    // case (thin AABB) but a huge grey blob under the stress-default layout,
    // whose edges run diagonally. Per-instance uniforms batch-collapse on this
    // fork, so direction is baked per-pen and the canvas routes each edge to
    // the pen matching its slope sign.
    mod.draw.EdgeLine = mod.draw.DrawColor{
        flip: uniform(0.0)
        zoom: uniform(1.0)
        pixel: fn() {
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            let w = self.rect_size.x
            let h = self.rect_size.y
            let sx = mix(0.0, w, self.flip)
            let ex = mix(w, 0.0, self.flip)
            sdf.move_to(sx, 0.0)
            sdf.line_to(ex, h)
            // Thickness tracks zoom but never drops below a screen-space
            // hairline, so relationships stay legible at fit-zoom on a large
            // (stress-laid) model instead of thinning to nothing.
            sdf.stroke(self.color, max(1.2, 2.0 * self.zoom))
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
        draw_edge_up: mod.draw.EdgeLine{ color: atlas.text_dim }
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
    draw_edge_up: DrawColor,
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

/// The four node commands a radial reports. Handlers are logging stubs for now
/// (there is no node-editing command path yet -- mirrors the `tool_dock` mock).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NodeCommand {
    Open,
    Style,
    Markdown,
    Remove,
}

/// Map a radial-committed `LiveId` to a node command. `None` = not one of ours.
pub fn node_command_for(id: LiveId) -> Option<NodeCommand> {
    if id == live_id!(open) {
        Some(NodeCommand::Open)
    } else if id == live_id!(style) {
        Some(NodeCommand::Style)
    } else if id == live_id!(markdown) {
        Some(NodeCommand::Markdown)
    } else if id == live_id!(remove) {
        Some(NodeCommand::Remove)
    } else {
        None
    }
}

/// Canvas -> App action (same convention as `ToolDockAction`).
#[derive(Clone, Debug, Default)]
pub enum GraphCanvasAction {
    #[default]
    None,
    /// A right-press landed on a node: open the radial at `abs` for `node`.
    /// `node` is carried for a later task's node-scoped command dispatch --
    /// unread until then, same convention as `radial::HUB_RADIUS`.
    NodeMenu {
        abs: DVec2,
        #[allow(dead_code)]
        node: usize,
    },
    /// A primary click landed on a node: repoint the inspector at its
    /// classifier. Carries the `SceneNode::key` directly so `App` never re-maps
    /// an index.
    NodeSelect { key: String },
    /// A primary click landed on empty canvas: clear the inspector.
    NodeDeselect,
    /// A primary click landed on a node's overflow footer band: toggle its card
    /// expansion. Consumed — no selection change. Carries the `SceneNode::key`.
    ToggleExpand { key: String },
}

impl Widget for GraphCanvas {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, _scope: &mut Scope) {
        match event.hits_with_capture_overload(cx, self.draw_bg.area(), false) {
            Hit::FingerDown(fe) if fe.mouse_button() == Some(MouseButton::SECONDARY) => {
                let rects: Vec<waml::solve::Rect> =
                    self.scene.nodes.iter().map(|n| n.rect).collect();
                if let Some(node) = node_at(&rects, &self.camera, self.view_rect, fe.abs) {
                    let uid = self.widget_uid();
                    cx.widget_action(uid, GraphCanvasAction::NodeMenu { abs: fe.abs, node });
                }
            }
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
                    }
                }
                cx.set_cursor(MouseCursor::Grab);
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

        // Edge pens: feed zoom so the stroke thickens with the box, and bake
        // each pen's diagonal direction (per-instance uniforms batch-collapse on
        // this fork, so the two pens carry the two constant `flip` values).
        self.draw_edge_down.set_uniform(cx, live_id!(flip), &[0.0]);
        self.draw_edge_down
            .set_uniform(cx, live_id!(zoom), &[zoom as f32]);
        self.draw_edge_up.set_uniform(cx, live_id!(flip), &[1.0]);
        self.draw_edge_up
            .set_uniform(cx, live_id!(zoom), &[zoom as f32]);

        // Edges: the segment from source border to target border, stroked along
        // the diagonal of its bounding box (see EdgeLine). Arrow/adornment
        // styling is a fast-follow.
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
            // Bounding box of the segment: the two endpoints sit at opposite
            // corners, so EdgeLine strokes the matching diagonal. Inflated to at
            // least `thickness` per axis so axis-aligned edges keep a body.
            let min = dvec2(a.x.min(b.x), a.y.min(b.y));
            let max = dvec2(a.x.max(b.x), a.y.max(b.y));
            let seg = Rect {
                pos: min,
                size: dvec2(
                    (max.x - min.x).max(thickness),
                    (max.y - min.y).max(thickness),
                ),
            };
            // Route to the pen whose baked diagonal matches this segment's
            // slope: same-sign deltas run top-left->bottom-right (down), opposite
            // signs run top-right->bottom-left (up). Axis-aligned edges (one zero
            // delta) fall to `down`; the thickness-inflated quad hides the slope.
            let pen = if (b.x - a.x) * (b.y - a.y) >= 0.0 {
                &mut self.draw_edge_down
            } else {
                &mut self.draw_edge_up
            };
            pen.draw_abs(cx, seg);
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

        DrawStep::done()
    }
}

impl GraphCanvas {
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
    fn node_command_maps_the_four_committed_ids() {
        assert_eq!(node_command_for(live_id!(open)), Some(NodeCommand::Open));
        assert_eq!(node_command_for(live_id!(style)), Some(NodeCommand::Style));
        assert_eq!(
            node_command_for(live_id!(markdown)),
            Some(NodeCommand::Markdown)
        );
        assert_eq!(
            node_command_for(live_id!(remove)),
            Some(NodeCommand::Remove)
        );
        assert_eq!(node_command_for(live_id!(bogus)), None);
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
