use std::sync::Arc;

use makepad_widgets::{dvec2, Rect};
use waml_markdown_editor::{
    input::ScrollAnchor,
    layout::{
        CaretStop, GeometryElementId, GlyphCluster, LayoutElementId, LayoutSnapshot, VisualLine,
    },
    motion::{LayoutChangeCause, MotionConfig, MotionController, MotionCutReason},
    selection::{Affinity, TextPosition},
};
use waml_syntax::{DocumentRevision, SyntaxIdentity, TextChange, TextRange, TextSize};

fn t(value: usize) -> TextSize {
    TextSize::try_from_usize(value).unwrap()
}

fn range(start: usize, end: usize) -> TextRange {
    TextRange::new(t(start), t(end)).unwrap()
}

fn position(offset: usize) -> TextPosition {
    TextPosition::new(t(offset), Affinity::Before)
}

/// A stable geometry identity, so the same logical cluster matches across
/// snapshots exactly as the layout engine's ordinals do.
fn cluster_id(ordinal: u32) -> GeometryElementId {
    GeometryElementId {
        layout: LayoutElementId {
            owner: SyntaxIdentity::from_raw_for_test(1),
            fragment_ordinal: 0,
        },
        cluster_ordinal: ordinal,
    }
}

/// One body cluster at `y`, plus the caret stop the caret layer draws.
fn snapshot_at(y: f64) -> Arc<LayoutSnapshot> {
    let cluster = GlyphCluster::new(
        cluster_id(0),
        range(0, 4),
        Rect {
            pos: dvec2(10.0, y),
            size: dvec2(40.0, 18.0),
        },
        vec![
            CaretStop::new(position(0), dvec2(10.0, y)),
            CaretStop::new(position(4), dvec2(50.0, y)),
        ]
        .into(),
    );
    Arc::new(LayoutSnapshot::from_parts_for_test(
        DocumentRevision::INITIAL,
        dvec2(200.0, y + 400.0),
        vec![VisualLine::for_test(range(0, 4), y, 18.0)],
        vec![cluster],
        Vec::new(),
    ))
}

fn cluster_y(snapshot: &LayoutSnapshot) -> f64 {
    snapshot.glyph_clusters()[0].rect.pos.y
}

fn local_edit(bytes: usize) -> LayoutChangeCause {
    LayoutChangeCause::LocalEdit {
        changes: Arc::from([TextChange {
            old_range: range(0, 0),
            replacement: Arc::from("x".repeat(bytes)),
        }]),
    }
}

fn controller_at(from: f64, to: f64) -> (MotionController, Arc<LayoutSnapshot>) {
    let mut motion = MotionController::new(400.0);
    let target = snapshot_at(to);
    motion.commit(
        10.0,
        Some(snapshot_at(from)),
        target.clone(),
        local_edit(1),
        false,
        None,
        MotionConfig::default(),
    );
    (motion, target)
}

#[test]
fn out_cubic_transition_has_exact_start_midpoint_and_target() {
    let (mut motion, _) = controller_at(20.0, 60.0);
    assert_eq!(cluster_y(&motion.sample(10.000).layout), 20.0);

    // OutCubic(0.5) == 0.875, so the element sits at 20 + 40 * 0.875.
    let midpoint = motion.sample(10.050);
    assert!(
        (midpoint.progress - 0.875).abs() < 1e-9,
        "{}",
        midpoint.progress
    );
    assert!((cluster_y(&midpoint.layout) - 55.0).abs() < 1e-9);
    assert!(midpoint.active);

    let end = motion.sample(10.100);
    assert_eq!(cluster_y(&end.layout), 60.0);
    assert!(!end.active);
}

#[test]
fn every_layer_reads_the_same_eased_displacement_at_the_midpoint() {
    let (mut motion, _) = controller_at(20.0, 60.0);
    let frame = motion.sample(10.050);
    let cluster = &frame.layout.glyph_clusters()[0];
    // Cluster rect, caret stops, and the caret query all agree.
    assert!((cluster.rect.pos.y - 55.0).abs() < 1e-9);
    assert!(cluster
        .caret_stops
        .iter()
        .all(|stop| (stop.point.y - 55.0).abs() < 1e-9));
    let caret = frame.layout.source_to_point(position(0)).unwrap();
    assert!((caret.rect.pos.y - 55.0).abs() < 1e-9);
}

#[test]
fn reduced_motion_initial_load_external_replacement_and_resize_cut() {
    for (cause, reduced, expected) in [
        (local_edit(1), true, MotionCutReason::ReducedMotion),
        (
            LayoutChangeCause::InitialLoad,
            false,
            MotionCutReason::InitialLoad,
        ),
        (
            LayoutChangeCause::ExternalReplacement,
            false,
            MotionCutReason::ExternalReplacement,
        ),
        (
            LayoutChangeCause::ViewportResize,
            false,
            MotionCutReason::ViewportResize,
        ),
    ] {
        let mut motion = MotionController::new(400.0);
        let frame = motion.commit(
            10.0,
            Some(snapshot_at(20.0)),
            snapshot_at(60.0),
            cause,
            reduced,
            None,
            MotionConfig::default(),
        );
        assert_eq!(frame.cut_reason, Some(expected));
        assert_eq!(
            cluster_y(&frame.layout),
            60.0,
            "a cut shows target geometry"
        );
        assert!(!frame.active);
    }
}

#[test]
fn the_source_budget_boundary_is_exact() {
    let mut motion = MotionController::new(400.0);
    let frame = motion.commit(
        10.0,
        Some(snapshot_at(20.0)),
        snapshot_at(60.0),
        local_edit(4096),
        false,
        None,
        MotionConfig::default(),
    );
    assert_eq!(frame.cut_reason, None, "4096 bytes still animates");

    let mut motion = MotionController::new(400.0);
    let frame = motion.commit(
        10.0,
        Some(snapshot_at(20.0)),
        snapshot_at(60.0),
        local_edit(4097),
        false,
        None,
        MotionConfig::default(),
    );
    assert_eq!(frame.cut_reason, Some(MotionCutReason::SourceBudget));
}

#[test]
fn the_visible_element_budget_boundary_is_exact() {
    let budget_of = |max: usize| MotionConfig {
        max_changed_visible_elements: max,
        ..MotionConfig::default()
    };
    // One cluster moves, so a budget of one animates and a budget of zero cuts.
    let mut motion = MotionController::new(400.0);
    assert_eq!(
        motion
            .commit(
                10.0,
                Some(snapshot_at(20.0)),
                snapshot_at(60.0),
                local_edit(1),
                false,
                None,
                budget_of(1),
            )
            .cut_reason,
        None
    );
    let mut motion = MotionController::new(400.0);
    assert_eq!(
        motion
            .commit(
                10.0,
                Some(snapshot_at(20.0)),
                snapshot_at(60.0),
                local_edit(1),
                false,
                None,
                budget_of(0),
            )
            .cut_reason,
        Some(MotionCutReason::VisibleGeometryBudget)
    );
}

#[test]
fn an_unchanged_geometry_change_reports_outside_viewport() {
    let mut motion = MotionController::new(400.0);
    let frame = motion.commit(
        10.0,
        Some(snapshot_at(20.0)),
        snapshot_at(20.0),
        local_edit(1),
        false,
        None,
        MotionConfig::default(),
    );
    assert_eq!(frame.cut_reason, Some(MotionCutReason::OutsideViewport));
}

#[test]
fn an_interrupted_transition_rebases_from_the_frame_on_screen() {
    let (mut motion, _) = controller_at(20.0, 60.0);
    // Interrupt at the midpoint, where the element sits at 55.
    let frame = motion.commit(
        10.050,
        None,
        snapshot_at(100.0),
        local_edit(1),
        false,
        None,
        MotionConfig::default(),
    );
    assert!(frame.cut_reason.is_none());
    assert!(
        (cluster_y(&frame.layout) - 55.0).abs() < 1e-9,
        "starts where it was"
    );
    assert_eq!(cluster_y(&motion.sample(10.150).layout), 100.0);
}

#[test]
fn scroll_follows_the_interpolated_caret_of_the_same_frame() {
    let mut motion = MotionController::new(400.0);
    motion.commit(
        10.0,
        Some(snapshot_at(120.0)),
        snapshot_at(200.0),
        local_edit(1),
        false,
        Some(ScrollAnchor {
            position: position(0),
            viewport_y: 20.0,
        }),
        MotionConfig::default(),
    );
    let midpoint = motion.sample(10.050);
    let caret = midpoint.layout.source_to_point(position(0)).unwrap();
    assert!((midpoint.scroll_y - (caret.rect.pos.y - 20.0)).abs() < 1e-9);
    let end = motion.sample(10.100);
    assert!((end.scroll_y - 180.0).abs() < 1e-9);
}

#[test]
fn new_elements_start_at_target_and_deleted_elements_are_absent() {
    let previous = snapshot_at(20.0);
    let mut target_clusters = target_two_clusters();
    let target = Arc::new(LayoutSnapshot::from_parts_for_test(
        DocumentRevision::INITIAL,
        dvec2(200.0, 400.0),
        vec![VisualLine::for_test(range(0, 8), 60.0, 18.0)],
        std::mem::take(&mut target_clusters),
        Vec::new(),
    ));
    let mut motion = MotionController::new(400.0);
    motion.commit(
        10.0,
        Some(previous),
        target,
        local_edit(1),
        false,
        None,
        MotionConfig::default(),
    );
    let frame = motion.sample(10.050);
    let clusters = frame.layout.glyph_clusters();
    assert_eq!(clusters.len(), 2, "only target elements exist");
    // The surviving cluster interpolates; the new one is already at target.
    assert!((clusters[0].rect.pos.y - 55.0).abs() < 1e-9);
    assert_eq!(clusters[1].rect.pos.y, 60.0);
}

fn target_two_clusters() -> Vec<GlyphCluster> {
    let surviving = snapshot_at(60.0).glyph_clusters()[0].clone();
    let mut fresh = surviving.clone();
    fresh.id = cluster_id(1);
    fresh.source_range = range(4, 8);
    fresh.rect.pos.x = 60.0;
    vec![surviving, fresh]
}
