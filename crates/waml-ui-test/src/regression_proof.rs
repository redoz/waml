//! Proof that the rendering gate catches the regression it was built for.
//!
//! A gate is only worth its maintenance if it would have fired on something
//! real. The case on record is `90ffcf0f`, which fixed five router defects and
//! moved two authored behavior goldens with them:
//!
//! * the state machine's `Active` self-loop went from 16px to 24px of border
//!   clearance (`137` -> `145` on its outbound leg);
//! * the `Active -> Idle` back edge shifted 8px off its midpoint, on both its
//!   horizontal legs (`304` -> `312` and `132` -> `140`).
//!
//! Nothing looked at those. `crates/waml`'s solver goldens moved, which pins
//! the numbers, but no assertion and no human ever compared the two DRAWINGS
//! -- visual sign-off ledger row V14 is still owed on exactly this change.
//!
//! This module takes the two route sets verbatim out of the repo's own
//! history, inks each one at the pen width the behavior canvas strokes
//! connectors with, and puts the pair through [`crate::reference`]'s real
//! comparator. The rasteriser here is a TEST FIXTURE, not the gate: it exists
//! only to turn two known geometries into two images that differ exactly as
//! the regression differed. The thing under test is the comparator's verdict.
//!
//! What this establishes is a magnitude, not a mechanism: a change of this
//! size, in a canvas of this size, lands well clear of
//! [`crate::reference::MAX_CHANGED_INK_RATIO`], and the antialias and
//! toolchain noise the module docs are worried about lands well under it.

#![cfg(test)]

use crate::reference::{diff_ink, Image, InkMask, INK_THRESHOLD, MAX_CHANGED_INK_RATIO};

/// The behavior canvas the gate compares, in device pixels: a 1000x786
/// logical surface at the headless window's dpi 2.
const CANVAS: (usize, usize) = (2000, 1572);
/// World-to-device scale and origin, chosen so the ~180x440 state-machine
/// scene sits inside the canvas the way the viewport places it.
const SCALE: f64 = 2.0;
const ORIGIN: (f64, f64) = (700.0, 300.0);
/// `Pen::REGULAR` is 2.0 logical px for every connector, which quantises to
/// 4 device px at dpi 2.
const CONNECTOR_PX: usize = 4;

const BACKGROUND: [u8; 3] = [250, 249, 246];
const INK: [u8; 3] = [32, 34, 38];

/// The state machine's routes before `90ffcf0f`, from
/// `git show 90ffcf0f^:crates/waml/tests/fixtures/behavior/state-machine/states.golden.txt`.
const BEFORE: &[&[(f64, f64)]] = &[
    // Active -> Active, the self-loop, at 16px of clearance.
    &[
        (121.0, 280.0),
        (137.0, 280.0),
        (137.0, 327.0),
        (121.0, 327.0),
    ],
    // Active -> Idle, the back edge, on its midpoint.
    &[
        (121.0, 304.0),
        (163.0, 304.0),
        (163.0, 132.0),
        (131.0, 132.0),
    ],
    &[(65.0, 18.0), (65.0, 74.0)],
    &[(65.0, 190.0), (65.0, 246.0)],
    &[(65.0, 361.0), (65.0, 417.0)],
];

/// The same routes after `90ffcf0f`, from the fixture as it stands today.
const AFTER: &[&[(f64, f64)]] = &[
    // The self-loop, now at 24px of clearance.
    &[
        (121.0, 280.0),
        (145.0, 280.0),
        (145.0, 327.0),
        (121.0, 327.0),
    ],
    // The back edge, now 8px off the midpoint.
    &[
        (121.0, 312.0),
        (163.0, 312.0),
        (163.0, 140.0),
        (131.0, 140.0),
    ],
    &[(65.0, 18.0), (65.0, 74.0)],
    &[(65.0, 190.0), (65.0, 246.0)],
    &[(65.0, 361.0), (65.0, 417.0)],
];

/// The four node boxes, which both versions place identically. They are here
/// so the compared canvas carries the ink a real capture would, rather than
/// five bare lines: the changed-pixel RATIO only means something against a
/// realistic amount of ink.
const NODES: &[(f64, f64, f64, f64)] = &[
    (9.0, 246.0, 112.0, 116.0),
    (54.0, 417.0, 22.0, 22.0),
    (0.0, 74.0, 131.0, 116.0),
    (56.0, 0.0, 18.0, 18.0),
];

fn blank_canvas() -> Image {
    Image {
        width: CANVAS.0,
        height: CANVAS.1,
        rgb: BACKGROUND
            .iter()
            .copied()
            .cycle()
            .take(CANVAS.0 * CANVAS.1 * 3)
            .collect(),
    }
}

fn to_device(point: (f64, f64)) -> (f64, f64) {
    (point.0 * SCALE + ORIGIN.0, point.1 * SCALE + ORIGIN.1)
}

fn fill(image: &mut Image, x0: usize, y0: usize, x1: usize, y1: usize) {
    for y in y0..y1.min(image.height) {
        for x in x0..x1.min(image.width) {
            let base = (y * image.width + x) * 3;
            image.rgb[base..base + 3].copy_from_slice(&INK);
        }
    }
}

/// Ink one orthogonal segment as a band `CONNECTOR_PX` thick, centred on the
/// line -- the same arrangement `canvas::pen::fill_band_at` produces for a
/// flat fill, without borrowing its `Cx`.
fn segment(image: &mut Image, a: (f64, f64), b: (f64, f64)) {
    let half = CONNECTOR_PX / 2;
    if (a.0 - b.0).abs() >= (a.1 - b.1).abs() {
        let y = a.1.round() as usize;
        let (x0, x1) = (a.0.min(b.0).round() as usize, a.0.max(b.0).round() as usize);
        fill(image, x0, y.saturating_sub(half), x1 + 1, y + half);
    } else {
        let x = a.0.round() as usize;
        let (y0, y1) = (a.1.min(b.1).round() as usize, a.1.max(b.1).round() as usize);
        fill(image, x.saturating_sub(half), y0, x + half, y1 + 1);
    }
}

fn draw(routes: &[&[(f64, f64)]]) -> Image {
    let mut image = blank_canvas();
    for (x, y, w, h) in NODES.iter().copied() {
        let top_left = to_device((x, y));
        let bottom_right = to_device((x + w, y + h));
        // A card border: four bands, not a filled box.
        segment(&mut image, top_left, (bottom_right.0, top_left.1));
        segment(&mut image, (top_left.0, bottom_right.1), bottom_right);
        segment(&mut image, top_left, (top_left.0, bottom_right.1));
        segment(&mut image, (bottom_right.0, top_left.1), bottom_right);
    }
    for route in routes {
        for pair in route.windows(2) {
            segment(&mut image, to_device(pair[0]), to_device(pair[1]));
        }
    }
    image
}

fn mask(image: &Image) -> InkMask {
    InkMask::cut(image, BACKGROUND, INK_THRESHOLD)
}

/// The headline claim: had this gate existed before `90ffcf0f`, the router
/// fix would have failed it and a human would have been shown the two
/// drawings.
#[test]
fn the_router_fix_of_90ffcf0f_fails_the_rendering_gate() {
    let before = mask(&draw(BEFORE));
    let after = mask(&draw(AFTER));

    let diff = diff_ink(&before, &after).unwrap();

    assert!(
        !diff.within(MAX_CHANGED_INK_RATIO),
        "the gate missed the router change: {}",
        diff.summary()
    );
    // The exact number, so a later tolerance change has to argue with a
    // measurement rather than with a feeling. 2000 device pixels flip: the
    // back edge's two horizontal legs move clear of themselves, the vertical
    // leg joining them grows at both ends, and the self-loop's three legs all
    // move. The ratio that comes out is 6.4e-4 -- six times the tolerance on
    // a canvas carrying 12,678 ink pixels.
    assert_eq!(diff.changed, 2000, "{}", diff.summary());
    assert!(
        diff.ratio() > MAX_CHANGED_INK_RATIO * 5.0,
        "the router change should clear the tolerance several times over: {}",
        diff.summary()
    );
}

/// The self-loop moving on its own -- the smaller half of `90ffcf0f`, and the
/// one a coarser gate would miss. Its outbound leg is 47 world px long, so at
/// dpi 2 it is a ~94px line moving 16px.
#[test]
fn the_self_loop_clearance_change_alone_fails_the_rendering_gate() {
    let only_loop_moved: Vec<&[(f64, f64)]> = std::iter::once(AFTER[0])
        .chain(BEFORE[1..].iter().copied())
        .collect();
    let before = mask(&draw(BEFORE));
    let after = mask(&draw(&only_loop_moved));

    let diff = diff_ink(&before, &after).unwrap();

    assert!(
        !diff.within(MAX_CHANGED_INK_RATIO),
        "the self-loop change alone slipped through: {}",
        diff.summary()
    );
}

/// The other side of the claim: the same scene drawn twice is not a
/// regression, so the gate is not simply failing everything.
#[test]
fn the_same_routes_drawn_twice_pass_the_rendering_gate() {
    let diff = diff_ink(&mask(&draw(AFTER)), &mask(&draw(AFTER))).unwrap();

    assert_eq!(diff.changed, 0);
    assert!(diff.within(MAX_CHANGED_INK_RATIO));
}

/// The reference for a scene this size has to stay committable. This is the
/// number the repo actually pays for one gated canvas.
#[test]
fn a_reference_for_this_scene_stays_under_thirty_kilobytes() {
    let stored = mask(&draw(AFTER)).encode("linux-x86_64");

    assert!(
        stored.len() < 30 * 1024,
        "a state-machine reference stored as {} bytes",
        stored.len()
    );
}
