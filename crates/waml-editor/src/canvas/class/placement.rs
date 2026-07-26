use super::interaction::{is_click, node_at};
use super::{FrameCommand, InteractionEffects, SurfaceIntent, TimerCommand};
use crate::canvas::viewport::{ease_out, Camera, ViewportController, MIN_ZOOM};
use crate::scene::{Scene, SceneEdge};
use makepad_widgets::{dvec2, DVec2, LiveId};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Placed {
    pub dir: Option<waml::syntax::Direction>,
}

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

pub const COMPASS_ZONES: [Zone; 8] = [
    Zone::Left,
    Zone::Right,
    Zone::Top,
    Zone::Bottom,
    Zone::TopLeft,
    Zone::TopRight,
    Zone::BottomLeft,
    Zone::BottomRight,
];

pub const DIAL_ZONES: [Zone; 8] = [
    Zone::Top,
    Zone::TopRight,
    Zone::Right,
    Zone::BottomRight,
    Zone::Bottom,
    Zone::BottomLeft,
    Zone::Left,
    Zone::TopLeft,
];

pub fn zone_id(zone: Zone) -> LiveId {
    match zone {
        Zone::Top => makepad_widgets::live_id!(place_top),
        Zone::TopRight => makepad_widgets::live_id!(place_top_right),
        Zone::Right => makepad_widgets::live_id!(place_right),
        Zone::BottomRight => makepad_widgets::live_id!(place_bottom_right),
        Zone::Bottom => makepad_widgets::live_id!(place_bottom),
        Zone::BottomLeft => makepad_widgets::live_id!(place_bottom_left),
        Zone::Left => makepad_widgets::live_id!(place_left),
        Zone::TopLeft => makepad_widgets::live_id!(place_top_left),
    }
}

pub fn zone_of_id(id: LiveId) -> Option<Zone> {
    DIAL_ZONES.into_iter().find(|&zone| zone_id(zone) == id)
}

pub fn zone_arrow(zone: Zone) -> crate::icons::Icon {
    use crate::icons::Icon::*;
    match zone {
        Zone::Top => ArrowUp,
        Zone::TopRight => ArrowUpRight,
        Zone::Right => ArrowRight,
        Zone::BottomRight => ArrowDownRight,
        Zone::Bottom => ArrowDown,
        Zone::BottomLeft => ArrowDownLeft,
        Zone::Left => ArrowLeft,
        Zone::TopLeft => ArrowUpLeft,
    }
}

pub fn zone_placed(zone: Zone) -> Placed {
    use waml::syntax::Direction::*;
    let dir = match zone {
        Zone::Left => LeftOf,
        Zone::Right => RightOf,
        Zone::Top => Above,
        Zone::Bottom => Below,
        Zone::TopLeft => AboveLeft,
        Zone::TopRight => AboveRight,
        Zone::BottomLeft => BelowLeft,
        Zone::BottomRight => BelowRight,
    };
    Placed { dir: Some(dir) }
}

#[derive(Clone, Debug)]
struct DialPair {
    subject_key: String,
    subject_title: String,
    reference_key: String,
    reference_title: String,
}

#[derive(Clone, Debug)]
pub struct DialPlacement {
    pub subject_key: String,
    pub subject_title: String,
    pub reference_key: String,
    pub reference_title: String,
    pub directions: Vec<waml::syntax::Direction>,
}

struct Preview {
    zone: Zone,
    from: Vec<waml::solve::Rect>,
    to: Vec<waml::solve::Rect>,
    baseline: Vec<waml::solve::Rect>,
    baseline_edges: Vec<SceneEdge>,
    edge_ends: Vec<Option<(usize, usize)>>,
    t: f64,
    zoom_from: f64,
    zoom_to: f64,
    cam_baseline: Camera,
    closing: bool,
    cam_from: Camera,
    ghost_b_center: DVec2,
    ghost_b_size: DVec2,
    ghost_b_key: String,
}

pub(super) const DIAL_REACH: f64 = crate::popup::radial::DISC_RADIUS + 72.0;
pub(super) const PREVIEW_SECS: f64 = 0.22;
pub(super) const DWELL_SECS: f64 = 0.18;

#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct PreviewGhost {
    pub(super) center: DVec2,
    pub(super) size: DVec2,
    pub(super) key: String,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct PlacementSnapshot {
    pub(super) dragged_key: Option<String>,
    pub(super) drag_moved: bool,
    pub(super) ghost: Option<waml::solve::Rect>,
    pub(super) armed_target_key: Option<String>,
    pub(super) compass_zone: Option<Zone>,
    pub(super) dial_center: Option<DVec2>,
    pub(super) conflict_zones: Vec<Zone>,
    pub(super) placed: Placed,
    pub(super) preview_ghost: Option<PreviewGhost>,
}

#[derive(Default)]
pub(super) struct PlacementInteraction {
    dragged_key: Option<String>,
    cached_drag_index: Option<usize>,
    grab_offset: (f64, f64),
    drag_moved: bool,
    ghost: Option<waml::solve::Rect>,
    dwell_candidate_key: Option<String>,
    armed_target_key: Option<String>,
    compass_zone: Option<Zone>,
    dial_center: Option<DVec2>,
    dial_pair: Option<DialPair>,
    candidate_layouts: Vec<(Zone, BTreeMap<String, waml::solve::Rect>)>,
    conflict_zones: Vec<Zone>,
    preview: Option<Preview>,
    preview_last_time: f64,
    pub(super) cursor_abs: DVec2,
}

fn resolve_index(scene: &Scene, key: &str) -> Option<usize> {
    scene.nodes.iter().position(|node| node.key == key)
}

pub(super) fn lerp_rect(a: waml::solve::Rect, b: waml::solve::Rect, t: f64) -> waml::solve::Rect {
    waml::solve::Rect {
        x: a.x + (b.x - a.x) * t,
        y: a.y + (b.y - a.y) * t,
        w: a.w + (b.w - a.w) * t,
        h: a.h + (b.h - a.h) * t,
    }
}

pub(super) fn preview_zoom(
    a: waml::solve::Rect,
    b: waml::solve::Rect,
    view: DVec2,
    pad: f64,
    start: f64,
) -> f64 {
    let min_x = a.x.min(b.x);
    let min_y = a.y.min(b.y);
    let max_x = (a.x + a.w).max(b.x + b.w);
    let max_y = (a.y + a.h).max(b.y + b.h);
    let (width, height) = ((max_x - min_x).max(1.0), (max_y - min_y).max(1.0));
    let fit = ((view.x - 2.0 * pad).max(1.0) / width).min((view.y - 2.0 * pad).max(1.0) / height);
    let ceiling = (start * 1.25).min(1.0);
    fit.clamp(MIN_ZOOM, ceiling.max(MIN_ZOOM))
}

impl PlacementInteraction {
    pub(super) fn begin_drag(&mut self, key: &str, abs: DVec2, grab_offset: (f64, f64)) {
        self.dragged_key = Some(key.to_string());
        self.cached_drag_index = None;
        self.grab_offset = grab_offset;
        self.drag_moved = false;
        self.ghost = None;
        self.dwell_candidate_key = None;
        self.cursor_abs = abs;
    }

    pub(super) fn drag_to(
        &mut self,
        abs: DVec2,
        scene: &mut Scene,
        viewport: &mut ViewportController,
    ) -> InteractionEffects {
        let Some(key) = self.dragged_key.clone() else {
            return InteractionEffects::default();
        };
        let index = match self
            .cached_drag_index
            .filter(|&index| scene.nodes.get(index).is_some_and(|node| node.key == key))
        {
            Some(index) => index,
            None => {
                let Some(index) = resolve_index(scene, &key) else {
                    return self.cancel(scene, viewport);
                };
                self.cached_drag_index = Some(index);
                index
            }
        };

        if !self.drag_moved && !is_click(self.cursor_abs, abs) {
            self.drag_moved = true;
        }
        if self.drag_moved {
            self.cursor_abs = abs;
        }

        let mut dismissed_dial = false;
        if let Some(center) = self.dial_center {
            if (abs - center).length() > DIAL_REACH {
                self.close_dial(scene, viewport);
                dismissed_dial = true;
            }
        }

        let viewport_snapshot = viewport.snapshot();
        let (world_x, world_y) = viewport_snapshot.camera.local_to_world(
            abs.x - viewport_snapshot.view_rect.pos.x,
            abs.y - viewport_snapshot.view_rect.pos.y,
        );
        let base = scene.nodes[index].rect;
        let ghost = waml::solve::Rect {
            x: world_x - self.grab_offset.0,
            y: world_y - self.grab_offset.1,
            w: base.w,
            h: base.h,
        };
        if self.dial_center.is_some() {
            if self.preview.is_some() {
                self.apply_preview_camera(scene, viewport);
                self.ghost = Some(scene.nodes[index].rect);
            } else {
                self.ghost = Some(ghost);
            }
            return InteractionEffects {
                consumed: true,
                redraw: true,
                ..Default::default()
            };
        }
        let hovered = node_at(&scene.nodes, viewport_snapshot, abs)
            .filter(|&target| target != index)
            .map(|target| scene.nodes[target].key.clone());
        let mut effects = self.hover_target(hovered.as_deref(), scene);
        self.compass_zone = None;
        self.ghost = Some(ghost);
        effects.consumed = true;
        effects.redraw = true;
        if dismissed_dial {
            effects.preview_frame = FrameCommand::Stop;
            effects.intent = Some(SurfaceIntent::DialDismiss);
        }
        effects
    }

    pub(super) fn hover_target(&mut self, key: Option<&str>, scene: &Scene) -> InteractionEffects {
        let key = key.filter(|key| {
            self.dragged_key.as_deref() != Some(*key) && resolve_index(scene, key).is_some()
        });
        if key == self.armed_target_key.as_deref() {
            let stopped = self.dwell_candidate_key.take().is_some();
            return InteractionEffects {
                dwell_timer: if stopped {
                    TimerCommand::Stop
                } else {
                    TimerCommand::Keep
                },
                ..Default::default()
            };
        }
        match key {
            Some(key) if self.dwell_candidate_key.as_deref() != Some(key) => {
                let restart = self.dwell_candidate_key.replace(key.to_string()).is_some();
                InteractionEffects {
                    dwell_timer: if restart {
                        TimerCommand::RestartTimeout(DWELL_SECS)
                    } else {
                        TimerCommand::StartTimeout(DWELL_SECS)
                    },
                    ..Default::default()
                }
            }
            None if self.dwell_candidate_key.take().is_some() => InteractionEffects {
                dwell_timer: TimerCommand::Stop,
                ..Default::default()
            },
            _ => InteractionEffects::default(),
        }
    }

    pub(super) fn dwell_elapsed(&mut self, scene: &Scene, center: DVec2) -> InteractionEffects {
        let (Some(subject_key), Some(reference_key)) =
            (self.dragged_key.clone(), self.dwell_candidate_key.take())
        else {
            return InteractionEffects::default();
        };
        let (Some(subject_index), Some(reference_index)) = (
            resolve_index(scene, &subject_key),
            resolve_index(scene, &reference_key),
        ) else {
            return InteractionEffects::default();
        };
        if subject_index == reference_index {
            return InteractionEffects::default();
        }
        self.cached_drag_index = Some(subject_index);
        self.armed_target_key = Some(reference_key.clone());
        self.dial_pair = Some(DialPair {
            subject_key: subject_key.clone(),
            subject_title: scene.nodes[subject_index].title.clone(),
            reference_key: reference_key.clone(),
            reference_title: scene.nodes[reference_index].title.clone(),
        });
        self.conflict_zones.clear();
        self.candidate_layouts.clear();
        self.dial_center = Some(center);
        InteractionEffects {
            redraw: true,
            intent: Some(SurfaceIntent::CompassArmed {
                subject_key,
                reference_key,
                center,
            }),
            ..Default::default()
        }
    }

    pub(super) fn set_candidate_layouts(
        &mut self,
        layouts: Vec<(Zone, BTreeMap<String, waml::solve::Rect>)>,
        scene: &mut Scene,
        viewport: &mut ViewportController,
    ) -> InteractionEffects {
        self.candidate_layouts = layouts;
        let mut effects = InteractionEffects {
            redraw: true,
            ..Default::default()
        };
        if let Some(zone) = self.compass_zone {
            if self.latch_preview(zone, scene, viewport) {
                effects.preview_frame = FrameCommand::Request;
            }
        }
        effects
    }

    pub(super) fn set_conflict_zones(&mut self, zones: Vec<Zone>) -> bool {
        if self.conflict_zones == zones {
            return false;
        }
        self.conflict_zones = zones;
        true
    }

    pub(super) fn preview_zone(
        &mut self,
        zone: Option<Zone>,
        scene: &mut Scene,
        viewport: &mut ViewportController,
    ) -> InteractionEffects {
        if zone == self.compass_zone {
            return InteractionEffects::default();
        }
        self.compass_zone = zone;
        let changed = match zone {
            Some(zone) => self.latch_preview(zone, scene, viewport),
            None => self.unlatch_preview_animated(scene, viewport),
        };
        InteractionEffects {
            redraw: true,
            preview_frame: if changed {
                FrameCommand::Request
            } else {
                FrameCommand::Keep
            },
            ..Default::default()
        }
    }

    pub(super) fn tick_preview(
        &mut self,
        time: f64,
        scene: &mut Scene,
        viewport: &mut ViewportController,
    ) -> InteractionEffects {
        let delta = if self.preview_last_time == 0.0 {
            0.0
        } else {
            time - self.preview_last_time
        };
        self.preview_last_time = time;
        let tweening = match &mut self.preview {
            Some(preview) => {
                preview.t = (preview.t + delta / PREVIEW_SECS).min(1.0);
                preview.t < 1.0
            }
            None => {
                self.preview_last_time = 0.0;
                return InteractionEffects {
                    preview_frame: FrameCommand::Stop,
                    ..Default::default()
                };
            }
        };
        self.apply_preview(scene, viewport);
        let closing_done =
            !tweening && self.preview.as_ref().is_some_and(|preview| preview.closing);
        if closing_done {
            self.unlatch_preview(scene, viewport);
        }
        if !tweening {
            self.preview_last_time = 0.0;
        }
        InteractionEffects {
            redraw: true,
            preview_frame: if tweening {
                FrameCommand::Request
            } else {
                FrameCommand::Stop
            },
            ..Default::default()
        }
    }

    pub(super) fn finish_pointer_up(
        &mut self,
        scene: &mut Scene,
        viewport: &mut ViewportController,
    ) -> InteractionEffects {
        self.close_dial(scene, viewport);
        self.dragged_key = None;
        self.cached_drag_index = None;
        self.drag_moved = false;
        self.ghost = None;
        self.dwell_candidate_key = None;
        InteractionEffects {
            consumed: true,
            redraw: true,
            dwell_timer: TimerCommand::Stop,
            preview_frame: FrameCommand::Stop,
            ..Default::default()
        }
    }

    pub(super) fn cancel(
        &mut self,
        scene: &mut Scene,
        viewport: &mut ViewportController,
    ) -> InteractionEffects {
        self.close_dial(scene, viewport);
        self.dragged_key = None;
        self.cached_drag_index = None;
        self.grab_offset = (0.0, 0.0);
        self.drag_moved = false;
        self.ghost = None;
        self.dwell_candidate_key = None;
        self.cursor_abs = DVec2::default();
        InteractionEffects {
            consumed: true,
            redraw: true,
            dwell_timer: TimerCommand::Stop,
            preview_frame: FrameCommand::Stop,
            ..Default::default()
        }
    }

    pub(super) fn cancel_for_scene_change(
        &mut self,
        scene: &mut Scene,
        viewport: &mut ViewportController,
    ) -> InteractionEffects {
        let effects = self.cancel(scene, viewport);
        self.dial_pair = None;
        self.candidate_layouts.clear();
        self.conflict_zones.clear();
        effects
    }

    pub(super) fn placement_for(&self, zone: Zone) -> Option<DialPlacement> {
        let pair = self.dial_pair.as_ref()?;
        let direction = zone_placed(zone).dir?;
        Some(DialPlacement {
            subject_key: pair.subject_key.clone(),
            subject_title: pair.subject_title.clone(),
            reference_key: pair.reference_key.clone(),
            reference_title: pair.reference_title.clone(),
            directions: vec![direction],
        })
    }

    pub(super) fn snapshot(&self) -> PlacementSnapshot {
        PlacementSnapshot {
            dragged_key: self.dragged_key.clone(),
            drag_moved: self.drag_moved,
            ghost: self.ghost,
            armed_target_key: self.armed_target_key.clone(),
            compass_zone: self.compass_zone,
            dial_center: self.dial_center,
            conflict_zones: self.conflict_zones.clone(),
            placed: self.compass_zone.map(zone_placed).unwrap_or_default(),
            preview_ghost: self.preview.as_ref().map(|preview| PreviewGhost {
                center: preview.ghost_b_center,
                size: preview.ghost_b_size,
                key: preview.ghost_b_key.clone(),
            }),
        }
    }

    fn latch_preview(
        &mut self,
        zone: Zone,
        scene: &mut Scene,
        viewport: &mut ViewportController,
    ) -> bool {
        if self
            .preview
            .as_ref()
            .is_some_and(|preview| preview.zone == zone && !preview.closing)
        {
            return false;
        }
        let (Some(subject_key), Some(reference_key)) = (
            self.dragged_key.as_deref(),
            self.armed_target_key.as_deref(),
        ) else {
            return false;
        };
        let (Some(subject_index), Some(reference_index)) = (
            resolve_index(scene, subject_key),
            resolve_index(scene, reference_key),
        ) else {
            self.close_dial(scene, viewport);
            return false;
        };
        self.cached_drag_index = Some(subject_index);
        let Some(layout) = self
            .candidate_layouts
            .iter()
            .find(|(candidate, _)| *candidate == zone)
            .map(|(_, layout)| layout.clone())
        else {
            return false;
        };
        let current: Vec<waml::solve::Rect> = scene.nodes.iter().map(|node| node.rect).collect();
        let to: Vec<waml::solve::Rect> = scene
            .nodes
            .iter()
            .enumerate()
            .map(|(index, node)| layout.get(&node.key).copied().unwrap_or(current[index]))
            .collect();
        let carried = self.preview.as_ref().map(|preview| {
            (
                preview.baseline.clone(),
                preview.baseline_edges.clone(),
                preview.edge_ends.clone(),
                preview.cam_baseline,
                preview.ghost_b_center,
                preview.ghost_b_size,
                preview.ghost_b_key.clone(),
            )
        });
        let (
            baseline,
            baseline_edges,
            edge_ends,
            cam_baseline,
            ghost_center,
            ghost_size,
            ghost_key,
        ) = match carried {
            Some(carried) => carried,
            None => {
                let edges = scene.edges.clone();
                let ends = scene
                    .edges
                    .iter()
                    .map(|edge| {
                        let find = |rect: waml::solve::Rect| {
                            current.iter().position(|candidate| {
                                (candidate.x - rect.x).abs() < 0.5
                                    && (candidate.y - rect.y).abs() < 0.5
                            })
                        };
                        Some((find(edge.source)?, find(edge.target)?))
                    })
                    .collect();
                let reference = scene.nodes[reference_index].rect;
                let (local_x, local_y) = viewport.camera().world_to_local(reference.x, reference.y);
                let screen_pos = viewport.snapshot().view_rect.pos + dvec2(local_x, local_y);
                (
                    current.clone(),
                    edges,
                    ends,
                    viewport.camera(),
                    screen_pos
                        + dvec2(
                            reference.w * viewport.camera().zoom * 0.5,
                            reference.h * viewport.camera().zoom * 0.5,
                        ),
                    dvec2(reference.w, reference.h),
                    scene.nodes[reference_index].key.clone(),
                )
            }
        };
        let zoom_to = preview_zoom(
            to[subject_index],
            to[reference_index],
            viewport.snapshot().view_rect.size,
            72.0,
            cam_baseline.zoom,
        );
        self.preview = Some(Preview {
            zone,
            from: current,
            to,
            baseline,
            baseline_edges,
            edge_ends,
            t: 0.0,
            zoom_from: viewport.camera().zoom,
            zoom_to,
            cam_baseline,
            closing: false,
            cam_from: viewport.camera(),
            ghost_b_center: ghost_center,
            ghost_b_size: ghost_size,
            ghost_b_key: ghost_key,
        });
        self.preview_last_time = 0.0;
        self.apply_preview(scene, viewport);
        true
    }

    fn unlatch_preview(&mut self, scene: &mut Scene, viewport: &mut ViewportController) -> bool {
        let Some(preview) = self.preview.take() else {
            return false;
        };
        for (node, rect) in scene.nodes.iter_mut().zip(preview.baseline.iter()) {
            node.rect = *rect;
        }
        for (edge, baseline) in scene.edges.iter_mut().zip(preview.baseline_edges.iter()) {
            *edge = baseline.clone();
        }
        viewport.set_transient_camera(preview.cam_baseline);
        self.preview_last_time = 0.0;
        if let Some(index) = self.dragged_index(scene) {
            self.ghost = Some(scene.nodes[index].rect);
        }
        true
    }

    fn unlatch_preview_animated(
        &mut self,
        scene: &mut Scene,
        viewport: &mut ViewportController,
    ) -> bool {
        let camera_now = viewport.camera();
        let Some(preview) = self.preview.as_mut() else {
            return false;
        };
        if preview.closing {
            return false;
        }
        let eased = ease_out(preview.t);
        preview.from = preview
            .from
            .iter()
            .zip(preview.to.iter())
            .map(|(from, to)| lerp_rect(*from, *to, eased))
            .collect();
        preview.to = preview.baseline.clone();
        preview.zoom_from = camera_now.zoom;
        preview.zoom_to = preview.cam_baseline.zoom;
        preview.cam_from = camera_now;
        preview.t = 0.0;
        preview.closing = true;
        self.preview_last_time = 0.0;
        self.apply_preview(scene, viewport);
        true
    }

    fn apply_preview(&mut self, scene: &mut Scene, viewport: &mut ViewportController) {
        let Some(preview) = &self.preview else {
            return;
        };
        let eased = ease_out(preview.t);
        let closing = preview.closing;
        let rects: Vec<waml::solve::Rect> = preview
            .from
            .iter()
            .zip(preview.to.iter())
            .map(|(from, to)| lerp_rect(*from, *to, eased))
            .collect();
        let ends = preview.edge_ends.clone();
        for (node, rect) in scene.nodes.iter_mut().zip(rects.iter()) {
            node.rect = *rect;
        }
        for (edge, end) in scene.edges.iter_mut().zip(ends.iter()) {
            if let Some((subject, reference)) = *end {
                let (subject_rect, reference_rect) = (rects[subject], rects[reference]);
                edge.source = subject_rect;
                edge.target = reference_rect;
                edge.points = vec![
                    (
                        subject_rect.x + subject_rect.w * 0.5,
                        subject_rect.y + subject_rect.h * 0.5,
                    ),
                    (
                        reference_rect.x + reference_rect.w * 0.5,
                        reference_rect.y + reference_rect.h * 0.5,
                    ),
                ];
            }
        }
        if closing {
            self.apply_preview_return_camera(viewport);
        } else {
            self.apply_preview_camera(scene, viewport);
        }
        if let Some(index) = self.dragged_index(scene) {
            self.ghost = Some(scene.nodes[index].rect);
        }
    }

    fn apply_preview_return_camera(&self, viewport: &mut ViewportController) {
        let Some(preview) = &self.preview else {
            return;
        };
        let eased = ease_out(preview.t);
        viewport.set_transient_camera(Camera {
            pan_x: preview.cam_from.pan_x
                + (preview.cam_baseline.pan_x - preview.cam_from.pan_x) * eased,
            pan_y: preview.cam_from.pan_y
                + (preview.cam_baseline.pan_y - preview.cam_from.pan_y) * eased,
            zoom: preview.cam_from.zoom
                + (preview.cam_baseline.zoom - preview.cam_from.zoom) * eased,
        });
    }

    fn apply_preview_camera(&self, scene: &Scene, viewport: &mut ViewportController) {
        let (Some(preview), Some(index)) = (&self.preview, self.dragged_index(scene)) else {
            return;
        };
        let zoom = preview.zoom_from + (preview.zoom_to - preview.zoom_from) * ease_out(preview.t);
        let dragged = scene.nodes[index].rect;
        let local = self.cursor_abs - viewport.snapshot().view_rect.pos;
        viewport.set_transient_camera(Camera {
            zoom,
            pan_x: dragged.x - local.x / zoom + self.grab_offset.0,
            pan_y: dragged.y - local.y / zoom + self.grab_offset.1,
        });
    }

    fn close_dial(&mut self, scene: &mut Scene, viewport: &mut ViewportController) {
        self.unlatch_preview(scene, viewport);
        self.dial_center = None;
        self.candidate_layouts.clear();
        self.conflict_zones.clear();
        self.armed_target_key = None;
        self.compass_zone = None;
    }

    fn dragged_index(&self, scene: &Scene) -> Option<usize> {
        let key = self.dragged_key.as_deref()?;
        self.cached_drag_index
            .filter(|&index| scene.nodes.get(index).is_some_and(|node| node.key == key))
            .or_else(|| resolve_index(scene, key))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::canvas::class::{FrameCommand, SurfaceIntent, TimerCommand};
    use crate::canvas::viewport::ViewportController;
    use crate::scene::{Scene, SceneNode};
    use makepad_widgets::{dvec2, Rect};
    use std::collections::BTreeMap;

    fn test_node(key: &str, x: f64) -> SceneNode {
        use waml::model::{ElementType, UmlMetaclass};
        SceneNode {
            key: key.to_string(),
            title: key.to_string(),
            element_type: ElementType::Uml(UmlMetaclass::Class),
            stereotypes: Vec::new(),
            attributes: Vec::new(),
            operations: Vec::new(),
            header: crate::scene::HeaderStyle::Plain,
            ports: false,
            rect: waml::solve::Rect {
                x,
                y: 0.0,
                w: 80.0,
                h: 60.0,
            },
            emphasized: false,
            collapsed: false,
            expanded: false,
        }
    }

    fn scene() -> Scene {
        Scene {
            display: Default::default(),
            nodes: vec![
                test_node("a", 0.0),
                test_node("b", 120.0),
                test_node("c", 240.0),
            ],
            groups: Vec::new(),
            edges: Vec::new(),
            relations: Vec::new(),
            conflicts: Vec::new(),
        }
    }

    fn scene_with_edge() -> Scene {
        use waml::model::{RelEnd, RelationshipKind};

        let mut scene = scene();
        scene.edges.push(crate::scene::SceneEdge {
            source: scene.nodes[0].rect,
            target: scene.nodes[1].rect,
            kind: RelationshipKind::Associates,
            from_end: RelEnd::default(),
            to_end: RelEnd::default(),
            points: vec![(40.0, 30.0), (40.0, 90.0), (160.0, 90.0), (160.0, 30.0)],
        });
        scene
    }

    fn scene_without(key: &str) -> Scene {
        let mut scene = scene();
        scene.nodes.retain(|node| node.key != key);
        scene
    }

    fn viewport() -> ViewportController {
        let mut viewport = ViewportController::default();
        viewport.set_view_rect(Rect {
            pos: dvec2(0.0, 0.0),
            size: dvec2(800.0, 600.0),
        });
        viewport
    }

    fn dragging(key: &str) -> PlacementInteraction {
        let mut placement = PlacementInteraction::default();
        placement.begin_drag(key, dvec2(10.0, 10.0), (2.0, 3.0));
        placement
    }

    fn preview_layout(a_x: f64, b_x: f64) -> BTreeMap<String, waml::solve::Rect> {
        BTreeMap::from([
            (
                "a".to_string(),
                waml::solve::Rect {
                    x: a_x,
                    y: 0.0,
                    w: 80.0,
                    h: 60.0,
                },
            ),
            (
                "b".to_string(),
                waml::solve::Rect {
                    x: b_x,
                    y: 0.0,
                    w: 80.0,
                    h: 60.0,
                },
            ),
        ])
    }

    fn assert_complete_preview_baseline(
        scene: &Scene,
        baseline: &Scene,
        viewport: &ViewportController,
        camera: Camera,
    ) {
        assert_eq!(
            scene.nodes.iter().map(|node| node.rect).collect::<Vec<_>>(),
            baseline
                .nodes
                .iter()
                .map(|node| node.rect)
                .collect::<Vec<_>>(),
            "node rects must roll back"
        );
        assert_eq!(
            scene.edges[0].source, baseline.edges[0].source,
            "edge source must roll back"
        );
        assert_eq!(
            scene.edges[0].target, baseline.edges[0].target,
            "edge target must roll back"
        );
        assert_eq!(
            scene.edges[0].points, baseline.edges[0].points,
            "edge route points must roll back"
        );
        assert_eq!(viewport.camera(), camera, "camera must roll back");
    }

    #[test]
    fn sub_slop_motion_never_starts_placement() {
        let mut placement = PlacementInteraction::default();
        placement.begin_drag("a", dvec2(10.0, 10.0), (2.0, 3.0));
        let mut scene = scene();
        let mut viewport = viewport();
        let effects = placement.drag_to(dvec2(13.0, 10.0), &mut scene, &mut viewport);
        assert!(!placement.snapshot().drag_moved);
        assert_eq!(effects.intent, None);
    }

    #[test]
    fn dwell_retarget_stops_the_old_timer_and_starts_a_new_one() {
        let mut placement = dragging("a");
        let first = placement.hover_target(Some("b"), &scene());
        assert_eq!(first.dwell_timer, TimerCommand::StartTimeout(DWELL_SECS));
        let second = placement.hover_target(Some("c"), &scene());
        assert_eq!(second.dwell_timer, TimerCommand::RestartTimeout(DWELL_SECS));
    }

    #[test]
    fn dwell_arm_emits_keys_and_frozen_center() {
        let mut placement = dragging("a");
        placement.hover_target(Some("b"), &scene());
        let effects = placement.dwell_elapsed(&scene(), dvec2(400.0, 300.0));
        assert_eq!(
            effects.intent,
            Some(SurfaceIntent::CompassArmed {
                subject_key: "a".into(),
                reference_key: "b".into(),
                center: dvec2(400.0, 300.0),
            }),
        );
    }

    #[test]
    fn scene_change_clears_stale_drag_dwell_dial_and_preview() {
        let mut placement = dragging("a");
        let mut scene = scene();
        let mut viewport = viewport();
        placement.hover_target(Some("b"), &scene);
        placement.dwell_elapsed(&scene, dvec2(400.0, 300.0));
        let layout = BTreeMap::from([
            (
                "a".to_string(),
                waml::solve::Rect {
                    x: 160.0,
                    y: 0.0,
                    w: 80.0,
                    h: 60.0,
                },
            ),
            (
                "b".to_string(),
                waml::solve::Rect {
                    x: 40.0,
                    y: 0.0,
                    w: 80.0,
                    h: 60.0,
                },
            ),
        ]);
        placement.set_candidate_layouts(vec![(Zone::Right, layout)], &mut scene, &mut viewport);
        placement.preview_zone(Some(Zone::Right), &mut scene, &mut viewport);
        let effects = placement.cancel_for_scene_change(&mut scene, &mut viewport);
        assert_eq!(placement.snapshot(), PlacementSnapshot::default());
        assert_eq!(effects.dwell_timer, TimerCommand::Stop);
        assert_eq!(effects.preview_frame, FrameCommand::Stop);
    }

    #[test]
    fn missing_keys_cancel_instead_of_using_cached_indices() {
        let mut placement = dragging("deleted");
        let mut scene = scene_without("deleted");
        let mut viewport = viewport();
        let effects = placement.drag_to(dvec2(40.0, 40.0), &mut scene, &mut viewport);
        assert_eq!(effects.intent, None);
        assert_eq!(placement.snapshot().dragged_key, None);
    }

    #[test]
    fn leaving_dial_reach_dismisses_and_falls_through_to_retarget_dwell() {
        let mut placement = dragging("a");
        let mut scene = scene();
        let mut viewport = viewport();
        placement.hover_target(Some("b"), &scene);
        placement.dwell_elapsed(&scene, dvec2(100.0, 100.0));
        placement.set_conflict_zones(vec![Zone::Left]);
        let over_c = dvec2(280.0, 30.0);
        assert!((over_c - dvec2(100.0, 100.0)).length() > DIAL_REACH);

        let effects = placement.drag_to(over_c, &mut scene, &mut viewport);

        assert_eq!(effects.intent, Some(SurfaceIntent::DialDismiss));
        assert_eq!(effects.preview_frame, FrameCommand::Stop);
        assert_eq!(effects.dwell_timer, TimerCommand::StartTimeout(DWELL_SECS));
        assert!(placement.snapshot().conflict_zones.is_empty());
        assert_eq!(placement.dwell_candidate_key.as_deref(), Some("c"));
        assert_eq!(
            placement.snapshot().ghost,
            Some(waml::solve::Rect {
                x: 278.0,
                y: 27.0,
                w: 80.0,
                h: 60.0,
            })
        );

        let armed = placement.dwell_elapsed(&scene, over_c);
        assert_eq!(
            armed.intent,
            Some(SurfaceIntent::CompassArmed {
                subject_key: "a".into(),
                reference_key: "c".into(),
                center: over_c,
            })
        );
    }

    #[test]
    fn leaving_a_preview_dial_retargets_after_restoring_baseline_geometry() {
        let mut placement = PlacementInteraction::default();
        placement.begin_drag("a", dvec2(2.0, 3.0), (2.0, 3.0));
        let mut scene = scene_with_edge();
        let baseline = scene.clone();
        let mut viewport = viewport();
        let camera = viewport.camera();
        placement.hover_target(Some("b"), &scene);
        placement.dwell_elapsed(&scene, dvec2(100.0, 100.0));
        let mut layout = preview_layout(-200.0, -80.0);
        layout.insert(
            "c".into(),
            waml::solve::Rect {
                x: 600.0,
                y: 0.0,
                w: 80.0,
                h: 60.0,
            },
        );
        placement.set_candidate_layouts(vec![(Zone::Right, layout)], &mut scene, &mut viewport);
        placement.preview_zone(Some(Zone::Right), &mut scene, &mut viewport);
        placement.tick_preview(10.0, &mut scene, &mut viewport);
        placement.tick_preview(10.0 + PREVIEW_SECS, &mut scene, &mut viewport);
        assert_eq!(scene.nodes[2].rect.x, 600.0);

        let over_baseline_c = dvec2(280.0, 30.0);
        assert!((over_baseline_c - dvec2(100.0, 100.0)).length() > DIAL_REACH);
        let effects = placement.drag_to(over_baseline_c, &mut scene, &mut viewport);

        assert_eq!(effects.intent, Some(SurfaceIntent::DialDismiss));
        assert_eq!(effects.preview_frame, FrameCommand::Stop);
        assert_eq!(effects.dwell_timer, TimerCommand::StartTimeout(DWELL_SECS));
        assert_eq!(placement.dwell_candidate_key.as_deref(), Some("c"));
        assert_eq!(
            placement.snapshot().ghost,
            Some(waml::solve::Rect {
                x: 278.0,
                y: 27.0,
                w: 80.0,
                h: 60.0,
            })
        );
        assert_complete_preview_baseline(&scene, &baseline, &viewport, camera);

        let armed = placement.dwell_elapsed(&scene, over_baseline_c);
        assert!(matches!(
            armed.intent,
            Some(SurfaceIntent::CompassArmed {
                subject_key,
                reference_key,
                center,
            }) if subject_key == "a"
                && reference_key == "c"
                && center == over_baseline_c
        ));
    }

    #[test]
    fn popup_commit_can_read_the_pair_after_pointer_up_teardown() {
        let mut placement = dragging("a");
        let mut scene = scene();
        let mut viewport = viewport();
        placement.hover_target(Some("b"), &scene);
        placement.dwell_elapsed(&scene, dvec2(100.0, 100.0));
        placement.finish_pointer_up(&mut scene, &mut viewport);
        let authored = placement.placement_for(Zone::Right).unwrap();
        assert_eq!(authored.subject_key, "a");
        assert_eq!(authored.reference_key, "b");
        assert_eq!(authored.directions, vec![waml::syntax::Direction::RightOf]);
    }

    #[test]
    fn preview_retargets_returns_and_clears() {
        let mut placement = dragging("a");
        let mut scene = scene();
        let mut viewport = viewport();
        placement.hover_target(Some("b"), &scene);
        placement.dwell_elapsed(&scene, dvec2(100.0, 100.0));
        let right = BTreeMap::from([
            (
                "a".to_string(),
                waml::solve::Rect {
                    x: 200.0,
                    y: 0.0,
                    w: 80.0,
                    h: 60.0,
                },
            ),
            (
                "b".to_string(),
                waml::solve::Rect {
                    x: 80.0,
                    y: 0.0,
                    w: 80.0,
                    h: 60.0,
                },
            ),
        ]);
        let left = BTreeMap::from([
            (
                "a".to_string(),
                waml::solve::Rect {
                    x: 0.0,
                    y: 0.0,
                    w: 80.0,
                    h: 60.0,
                },
            ),
            (
                "b".to_string(),
                waml::solve::Rect {
                    x: 120.0,
                    y: 0.0,
                    w: 80.0,
                    h: 60.0,
                },
            ),
        ]);
        placement.set_candidate_layouts(
            vec![(Zone::Right, right), (Zone::Left, left)],
            &mut scene,
            &mut viewport,
        );
        placement.preview_zone(Some(Zone::Right), &mut scene, &mut viewport);
        placement.preview_zone(Some(Zone::Left), &mut scene, &mut viewport);
        assert_eq!(
            placement.preview.as_ref().map(|preview| preview.zone),
            Some(Zone::Left)
        );
        placement.preview_zone(None, &mut scene, &mut viewport);
        assert!(placement
            .preview
            .as_ref()
            .is_some_and(|preview| preview.closing));
        placement.tick_preview(10.0, &mut scene, &mut viewport);
        placement.tick_preview(10.0 + PREVIEW_SECS, &mut scene, &mut viewport);
        assert!(placement.preview.is_none());
    }

    #[test]
    fn abrupt_cancel_restores_nodes_complete_edges_and_camera() {
        let mut placement = dragging("a");
        let mut scene = scene_with_edge();
        let mut viewport = viewport();
        viewport.set_transient_camera(Camera {
            pan_x: 12.0,
            pan_y: -5.0,
            zoom: 0.8,
        });
        placement.hover_target(Some("b"), &scene);
        placement.dwell_elapsed(&scene, dvec2(100.0, 100.0));
        placement.set_candidate_layouts(
            vec![(Zone::Right, preview_layout(200.0, 80.0))],
            &mut scene,
            &mut viewport,
        );
        let baseline = scene.clone();
        let camera = viewport.camera();

        placement.preview_zone(Some(Zone::Right), &mut scene, &mut viewport);
        placement.tick_preview(10.0, &mut scene, &mut viewport);
        placement.tick_preview(10.0 + PREVIEW_SECS * 0.5, &mut scene, &mut viewport);
        assert_ne!(scene.nodes[0].rect, baseline.nodes[0].rect);
        assert_ne!(scene.edges[0].source, baseline.edges[0].source);
        assert_ne!(viewport.camera(), camera);

        placement.cancel(&mut scene, &mut viewport);

        assert_complete_preview_baseline(&scene, &baseline, &viewport, camera);
    }

    #[test]
    fn successful_relatch_keeps_the_complete_original_rollback_baseline() {
        let mut placement = dragging("a");
        let mut scene = scene_with_edge();
        let mut viewport = viewport();
        viewport.set_transient_camera(Camera {
            pan_x: 12.0,
            pan_y: -5.0,
            zoom: 0.8,
        });
        placement.hover_target(Some("b"), &scene);
        placement.dwell_elapsed(&scene, dvec2(100.0, 100.0));
        placement.set_candidate_layouts(
            vec![
                (Zone::Right, preview_layout(200.0, 80.0)),
                (Zone::Left, preview_layout(-120.0, 120.0)),
            ],
            &mut scene,
            &mut viewport,
        );
        let baseline = scene.clone();
        let camera = viewport.camera();

        placement.preview_zone(Some(Zone::Right), &mut scene, &mut viewport);
        placement.tick_preview(20.0, &mut scene, &mut viewport);
        placement.tick_preview(20.0 + PREVIEW_SECS, &mut scene, &mut viewport);
        placement.preview_zone(Some(Zone::Left), &mut scene, &mut viewport);
        placement.tick_preview(30.0, &mut scene, &mut viewport);
        placement.tick_preview(30.0 + PREVIEW_SECS, &mut scene, &mut viewport);
        assert_eq!(
            placement.preview.as_ref().map(|preview| preview.zone),
            Some(Zone::Left)
        );
        assert_eq!(scene.nodes[0].rect.x, -120.0);
        assert_eq!(scene.edges[0].source, scene.nodes[0].rect);

        placement.cancel(&mut scene, &mut viewport);

        assert_complete_preview_baseline(&scene, &baseline, &viewport, camera);
    }

    #[test]
    fn escape_cancel_clears_drag_candidate_and_preview_state() {
        let mut placement = dragging("a");
        let mut scene = scene();
        let mut viewport = viewport();
        let effects = placement.cancel(&mut scene, &mut viewport);
        assert_eq!(placement.snapshot(), PlacementSnapshot::default());
        assert_eq!(effects.dwell_timer, TimerCommand::Stop);
    }
}
