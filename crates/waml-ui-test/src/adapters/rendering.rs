//! The rendering gate: capture the diagram canvas and hold it to a stored
//! reference.
//!
//! [`crate::reference`] is the comparator and carries the argument for why it
//! compares ink rather than pixel values, and why the stored reference is
//! text. This module is the plumbing around it: which region of the window is
//! compared, where references live, and -- the part that decides whether this
//! gate survives contact with CI -- when a mismatch is a failure rather than a
//! note.
//!
//! # Only the canvas
//!
//! The comparison is cropped to the visible diagram surface's rect, read from
//! the same frame the screenshot came from. A whole-window baseline would fail
//! on every tab title, chrome tweak and font change, which is how a rendering
//! gate becomes a chore and then becomes deleted. Cropping keeps the claim
//! narrow enough to be true: this is a gate on how the DIAGRAM is drawn.
//!
//! # The platform of record is Linux, and this is not a preference
//!
//! On Windows the fork's headless backend compiles every shader and then
//! cannot load any of them: `HeadlessLoadedModule::load` is `#[cfg(unix)]`,
//! and the `not(unix)` arm returns "headless shader dlopen is only
//! implemented on unix for now" (fork rev `6534634a`). The virtual GPU is
//! left with no fragment functions, so the framebuffer never leaves its clear
//! colour and every capture is a flat rectangle. Measured, not assumed: 218
//! shader DLLs on disk under `target/makepad-headless-jit/`, and a 2560x1680
//! screenshot with exactly one distinct colour in it.
//!
//! That is also why every "failure screenshot" a Windows run has ever
//! preserved is a blank image. [`BLANK_CANVAS_FLOOR`] is the guard, and it
//! reads the situation rather than blanket-failing:
//!
//! * blank while RECORDING -- hard error. A blank reference would be a gate
//!   that passes forever, so it can never be written by accident.
//! * blank while a reference EXISTS for this platform -- hard error. The
//!   renderer used to draw here and has stopped, which is a regression of
//!   exactly the kind this gate is for.
//! * blank with no reference for this platform -- advisory, naming the
//!   backend limitation. Failing here would leave a permanently red test on
//!   the owner's own machine over something no change of his can fix, and a
//!   permanently red test is a deleted test.
//!
//! # One reference per platform, and the honest gap
//!
//! Headless makepad rasterises on the CPU, but its shaders are JIT-compiled by
//! the host's own C toolchain, so two platforms do not produce the same
//! framebuffer. References are keyed by [`platform_key`], and the rule is:
//!
//! * a reference exists for this platform -- **enforced**, a mismatch fails
//!   the scenario;
//! * no reference exists for this platform -- **advisory**, the capture is
//!   recorded into the run's artifacts and the operation passes with a note
//!   naming the file to commit.
//!
//! That asymmetry is deliberate, and it is why this gate can be wired to CI on
//! the day it lands instead of turning main red. No reference ships in the
//! first commit because no machine that could record one was available to it:
//! the first Linux CI run records its own and uploads it as an artifact, and
//! the gate starts enforcing there the moment someone commits it. What is NOT
//! acceptable is a third state where a missing reference passes with no trace
//! -- so the advisory note goes into the semantic trace and the artifacts
//! both, and it says which file to commit.

use crate::error::OperationFailure;
use crate::reference::{diff_ink, Crop, Image, InkMask, INK_THRESHOLD, MAX_CHANGED_INK_RATIO};
use makepad_test::{TestApp, WidgetSnapshot};
use std::path::{Path, PathBuf};

/// Set to record every capture as the reference for this platform, including
/// ones that already have a reference. The gate never self-heals without it.
const UPDATE_ENV: &str = "WAML_UI_TEST_UPDATE_REFERENCES";

/// The fewest ink pixels a real diagram canvas can carry. Any state machine
/// the gate compares draws thousands; a capture under this floor is a
/// renderer that produced nothing, not a diagram that happens to be sparse.
const BLANK_CANVAS_FLOOR: usize = 500;

/// Which build of the rasteriser a reference was recorded against. The C
/// toolchain that JITs the shaders is what actually differs; OS plus arch is
/// the closest stable proxy for it.
pub(crate) fn platform_key() -> String {
    format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH)
}

pub(crate) fn reference_path(references_dir: &Path, name: &str) -> PathBuf {
    references_dir.join(format!("{name}.{}.ink", platform_key()))
}

pub(crate) fn expect_canvas_matches_reference(
    driver: &TestApp,
    references_dir: &Path,
    recordings_dir: &Path,
    artifacts_dir: &Path,
    name: &str,
) -> Result<String, OperationFailure> {
    let widgets = driver
        .try_widget_snapshot()
        .map_err(|error| OperationFailure {
            observed: "the canvas could not be located".to_string(),
            detail: error.message().to_string(),
        })?;
    let crop = canvas_crop(&widgets)?;

    let capture_path = driver.try_screenshot().map_err(|error| OperationFailure {
        observed: "the headless renderer produced no screenshot".to_string(),
        detail: error.message().to_string(),
    })?;
    let capture_bytes = std::fs::read(&capture_path).map_err(|error| OperationFailure {
        observed: "the screenshot could not be read back".to_string(),
        detail: format!("{}: {error}", capture_path.display()),
    })?;
    let capture = Image::decode_png(&capture_bytes).map_err(|error| OperationFailure {
        observed: "the screenshot could not be decoded".to_string(),
        detail: error.to_string(),
    })?;
    // A screenshot is in device pixels and a widget rect is in logical ones.
    // At dpi 1 they agree; the headless window runs at dpi 2, so the crop has
    // to be scaled by a factor read off the capture itself.
    let crop = scale_crop(crop, &capture, &widgets)?;
    let canvas = capture.crop(crop).map_err(|error| OperationFailure {
        observed: "the canvas rect does not lie inside the capture".to_string(),
        detail: error.to_string(),
    })?;

    let drawn_ink = InkMask::cut(&canvas, canvas.background(), INK_THRESHOLD).ink();
    let blank = drawn_ink < BLANK_CANVAS_FLOOR;
    let reference_file = reference_path(references_dir, name);
    let updating = std::env::var_os(UPDATE_ENV).is_some();
    match verdict_for(blank, updating, reference_file.is_file()) {
        Gate::RefuseBlank => return Err(blank_canvas(&canvas, drawn_ink)),
        Gate::Record => return record(&reference_file, references_dir, &canvas),
        Gate::AdviseBlank => {
            return Ok(format!(
                "ADVISORY: the {name} canvas capture is blank ({drawn_ink} ink pixels in \
                 {}x{}) and no {} reference exists, so the rendering gate did not run. The \
                 fork's headless shader loader is unix-only, so this platform's virtual GPU \
                 draws nothing; the gate's platform of record is Linux.",
                canvas.width,
                canvas.height,
                platform_key()
            ))
        }
        Gate::Advise => return Ok(advise(recordings_dir, &reference_file, name, &canvas)),
        Gate::Compare => {}
    }

    let reference_text =
        std::fs::read_to_string(&reference_file).map_err(|error| OperationFailure {
            observed: "the stored reference could not be read".to_string(),
            detail: format!("{}: {error}", reference_file.display()),
        })?;
    let reference = InkMask::decode(&reference_text).map_err(|error| OperationFailure {
        observed: "the stored reference could not be read as an ink mask".to_string(),
        detail: format!("{}: {error}", reference_file.display()),
    })?;
    // The capture is cut against the REFERENCE's background and threshold, so
    // a repainted canvas reads as every pixel changing rather than as a mask
    // that quietly re-referenced itself.
    let actual = InkMask::cut(&canvas, reference.background, reference.threshold);
    let diff = diff_ink(&reference, &actual).map_err(|error| OperationFailure {
        observed: format!("the {name} canvas cannot be compared to its reference"),
        detail: error.to_string(),
    })?;
    if diff.within(MAX_CHANGED_INK_RATIO) {
        return Ok(format!(
            "the {name} canvas matches its {} reference ({})",
            platform_key(),
            diff.summary()
        ));
    }
    // The capture, the recorded mask and a red/green overlay all go next to
    // the trace: the number alone never tells anyone whether the change was
    // wanted.
    let capture_file = write_png(artifacts_dir, &format!("{name}-actual.png"), &canvas);
    let overlay_file = write_png(
        artifacts_dir,
        &format!("{name}-diff.png"),
        &diff.visualization,
    );
    let recorded = write_text(
        artifacts_dir,
        &format!("{name}.{}.ink", platform_key()),
        &actual.encode(&platform_key()),
    );
    Err(OperationFailure {
        observed: format!(
            "the {name} canvas was drawn differently: {}",
            diff.summary()
        ),
        detail: format!(
            "tolerance is {MAX_CHANGED_INK_RATIO:.1e}. Reference: {}. Capture: {capture_file}. \
             Red/green overlay: {overlay_file} (red is ink that vanished, green is ink that \
             appeared). If the change is intended, copy {recorded} over the reference, or \
             re-run with {UPDATE_ENV}=1, and commit it.",
            reference_file.display()
        ),
    })
}

/// What to do with one capture. Split out from the flow so the decision is a
/// pure function with its own tests -- it is the whole policy of this gate,
/// and it is four lines of logic that would otherwise only be exercised by a
/// two-minute end-to-end run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Gate {
    /// Blank, and either recording or holding an existing reference: a hard
    /// error either way.
    RefuseBlank,
    /// Blank with nothing to compare against on a platform that cannot draw.
    AdviseBlank,
    Record,
    Advise,
    Compare,
}

fn verdict_for(blank: bool, updating: bool, reference_exists: bool) -> Gate {
    match (blank, updating, reference_exists) {
        (true, true, _) | (true, false, true) => Gate::RefuseBlank,
        (true, false, false) => Gate::AdviseBlank,
        (false, true, _) => Gate::Record,
        (false, false, false) => Gate::Advise,
        (false, false, true) => Gate::Compare,
    }
}

/// A capture with no ink in it. Named rather than tolerated, because the one
/// platform where this happens is also the one where the gate would otherwise
/// look like it was working.
fn blank_canvas(canvas: &Image, ink: usize) -> OperationFailure {
    let background = canvas.background();
    OperationFailure {
        observed: format!(
            "the canvas capture is blank: {ink} ink pixels in {}x{}, all of it {:?}",
            canvas.width, canvas.height, background
        ),
        detail: format!(
            "the headless renderer drew nothing, so there are no pixels to compare. On {} \
             this is expected: the fork's headless shader loader is unix-only \
             (`HeadlessLoadedModule::load`), so every shader compiles and none loads. The \
             rendering gate's platform of record is Linux.",
            platform_key()
        ),
    }
}

fn record(
    reference_file: &Path,
    references_dir: &Path,
    canvas: &Image,
) -> Result<String, OperationFailure> {
    std::fs::create_dir_all(references_dir).map_err(|error| OperationFailure {
        observed: "the reference directory could not be created".to_string(),
        detail: format!("{}: {error}", references_dir.display()),
    })?;
    let mask = InkMask::cut(canvas, canvas.background(), INK_THRESHOLD);
    let text = mask.encode(&platform_key());
    std::fs::write(reference_file, &text).map_err(|error| OperationFailure {
        observed: "the reference could not be written".to_string(),
        detail: format!("{}: {error}", reference_file.display()),
    })?;
    Ok(format!(
        "recorded {} ({} bytes, {}x{}, {} ink pixels) -- {UPDATE_ENV} was set",
        reference_file.display(),
        text.len(),
        mask.width,
        mask.height,
        mask.ink()
    ))
}

fn advise(recordings_dir: &Path, reference_file: &Path, name: &str, canvas: &Image) -> String {
    if let Err(error) = std::fs::create_dir_all(recordings_dir) {
        return format!(
            "ADVISORY: no {} reference for {name}, and the recording directory {} could not \
             be created: {error}",
            platform_key(),
            recordings_dir.display()
        );
    }
    let mask = InkMask::cut(canvas, canvas.background(), INK_THRESHOLD);
    let recorded = write_text(
        recordings_dir,
        &format!("{name}.{}.ink", platform_key()),
        &mask.encode(&platform_key()),
    );
    let png = write_png(recordings_dir, &format!("{name}-actual.png"), canvas);
    format!(
        "ADVISORY: no {} reference for {name}, so the rendering gate did not enforce here. \
         Recorded {recorded} ({} ink pixels) and {png} to look at; commit the mask as {} to \
         start enforcing on this platform.",
        platform_key(),
        mask.ink(),
        reference_file.display()
    )
}

fn write_png(artifacts_dir: &Path, file_name: &str, image: &Image) -> String {
    let path = artifacts_dir.join(file_name);
    match image
        .encode_png()
        .map_err(|error| error.to_string())
        .and_then(|bytes| std::fs::write(&path, bytes).map_err(|error| error.to_string()))
    {
        Ok(()) => path.display().to_string(),
        Err(error) => format!("<not written: {error}>"),
    }
}

fn write_text(artifacts_dir: &Path, file_name: &str, text: &str) -> String {
    let path = artifacts_dir.join(file_name);
    match std::fs::write(&path, text) {
        Ok(()) => path.display().to_string(),
        Err(error) => format!("<not written: {error}>"),
    }
}

/// The visible diagram surface's rect, in logical pixels.
///
/// There are two diagram surfaces -- the class canvas and the behavior canvas
/// -- and the inactive one stays in the tree as a hidden zero-rect view. So
/// this looks for exactly one VISIBLE surface rather than for one by name.
fn canvas_crop(widgets: &[WidgetSnapshot]) -> Result<Crop, OperationFailure> {
    let canvases: Vec<_> = widgets
        .iter()
        .filter(|widget| {
            crate::adapters::documents::DIAGRAM_SURFACE_IDS.contains(&widget.id.as_str())
                && widget.visible
        })
        .collect();
    let canvas = match canvases.as_slice() {
        [canvas] => *canvas,
        [] => {
            return Err(OperationFailure {
                observed: "no visible diagram canvas".to_string(),
                detail: "expected exactly one visible diagram surface".to_string(),
            })
        }
        found => {
            return Err(OperationFailure {
                observed: format!("{} visible diagram canvases", found.len()),
                detail: "the canvas selector is ambiguous".to_string(),
            })
        }
    };
    if canvas.width <= 0 || canvas.height <= 0 {
        return Err(OperationFailure {
            observed: format!("the canvas is {}x{}", canvas.width, canvas.height),
            detail: "a canvas with no area cannot be compared".to_string(),
        });
    }
    Ok(Crop {
        x: canvas.x.max(0) as usize,
        y: canvas.y.max(0) as usize,
        width: canvas.width as usize,
        height: canvas.height as usize,
    })
}

/// Rescale a logical-pixel crop into the capture's device pixels.
///
/// The scale is derived rather than assumed: the widest widget rect is the
/// window in logical pixels, and the capture is that same window in device
/// pixels. Assuming dpi 1 would have silently cropped a quarter of the canvas
/// on the headless window, which runs at dpi 2.
fn scale_crop(
    crop: Crop,
    capture: &Image,
    widgets: &[WidgetSnapshot],
) -> Result<Crop, OperationFailure> {
    let logical_width = widgets
        .iter()
        .map(|widget| widget.x.max(0) + widget.width.max(0))
        .max()
        .unwrap_or(0);
    if logical_width <= 0 {
        return Err(OperationFailure {
            observed: "the window reports no width".to_string(),
            detail: "cannot map widget rects onto the capture".to_string(),
        });
    }
    let scale = (capture.width as f64 / logical_width as f64)
        .round()
        .max(1.0);
    Ok(Crop {
        x: (crop.x as f64 * scale) as usize,
        y: (crop.y as f64 * scale) as usize,
        width: (crop.width as f64 * scale) as usize,
        height: (crop.height as f64 * scale) as usize,
    })
}

#[cfg(test)]
mod tests {
    use super::{canvas_crop, platform_key, reference_path, scale_crop, verdict_for, Gate};
    use crate::reference::{Crop, Image};
    use makepad_test::WidgetSnapshot;
    use std::path::Path;

    fn widget(id: &str, x: i64, y: i64, width: i64, height: i64, visible: bool) -> WidgetSnapshot {
        WidgetSnapshot {
            id: id.to_string(),
            widget_type: "View".to_string(),
            window_id: "main_window".to_string(),
            window_index: 0,
            visible,
            enabled: true,
            x,
            y,
            width,
            height,
            text: None,
            value: None,
            checked: None,
            selected: None,
        }
    }

    fn blank(width: usize, height: usize) -> Image {
        Image {
            width,
            height,
            rgb: vec![255; width * height * 3],
        }
    }

    #[test]
    fn the_visible_class_canvas_bounds_the_compared_region() {
        let widgets = vec![
            widget("root", 0, 0, 1280, 840, true),
            widget("canvas_wrap", 280, 30, 1000, 786, true),
            widget("behavior_canvas_wrap", 0, 0, 0, 0, false),
        ];

        let crop = canvas_crop(&widgets).unwrap();

        assert_eq!(
            crop,
            Crop {
                x: 280,
                y: 30,
                width: 1000,
                height: 786
            }
        );
    }

    /// The trap that made this scenario fail on its first run: a behavior
    /// document leaves `canvas_wrap` in the tree as a hidden zero-rect view,
    /// so a gate that looked up the class canvas by name found the wrong
    /// widget.
    #[test]
    fn the_visible_behavior_canvas_bounds_the_compared_region() {
        let widgets = vec![
            widget("root", 0, 0, 1280, 840, true),
            widget("canvas_wrap", 0, 0, 0, 0, false),
            widget("behavior_canvas_wrap", 280, 30, 1000, 786, true),
        ];

        let crop = canvas_crop(&widgets).unwrap();

        assert_eq!(crop.width, 1000);
        assert_eq!(crop.height, 786);
    }

    #[test]
    fn a_hidden_canvas_is_not_a_canvas() {
        let widgets = vec![widget("canvas_wrap", 0, 0, 900, 700, false)];

        let error = canvas_crop(&widgets).unwrap_err();

        assert_eq!(error.observed, "no visible diagram canvas");
    }

    /// The trap the crate docs already record: a widget can report itself
    /// visible and still occupy nothing. Comparing a zero-area region would
    /// pass forever.
    #[test]
    fn a_zero_area_canvas_is_rejected_rather_than_compared() {
        let widgets = vec![widget("canvas_wrap", 10, 10, 900, 0, true)];

        let error = canvas_crop(&widgets).unwrap_err();

        assert_eq!(error.observed, "the canvas is 900x0");
    }

    #[test]
    fn two_visible_canvases_are_ambiguous() {
        let widgets = vec![
            widget("canvas_wrap", 0, 0, 900, 700, true),
            widget("behavior_canvas_wrap", 0, 0, 900, 700, true),
        ];

        let error = canvas_crop(&widgets).unwrap_err();

        assert_eq!(error.observed, "2 visible diagram canvases");
    }

    #[test]
    fn a_crop_at_dpi_one_is_left_alone() {
        let widgets = vec![widget("root", 0, 0, 1280, 840, true)];
        let crop = Crop {
            x: 280,
            y: 30,
            width: 1000,
            height: 786,
        };

        let scaled = scale_crop(crop, &blank(1280, 840), &widgets).unwrap();

        assert_eq!(scaled, crop);
    }

    /// The headless window is dpi 2, which is the case that actually runs.
    #[test]
    fn a_crop_on_the_headless_dpi_two_capture_doubles() {
        let widgets = vec![widget("root", 0, 0, 1280, 840, true)];
        let crop = Crop {
            x: 280,
            y: 30,
            width: 1000,
            height: 786,
        };

        let scaled = scale_crop(crop, &blank(2560, 1680), &widgets).unwrap();

        assert_eq!(
            scaled,
            Crop {
                x: 560,
                y: 60,
                width: 2000,
                height: 1572
            }
        );
    }

    /// A blank reference would be a gate that passes forever, so recording
    /// one is refused however the run was launched.
    #[test]
    fn a_blank_capture_is_never_recorded_as_a_reference() {
        assert_eq!(verdict_for(true, true, false), Gate::RefuseBlank);
        assert_eq!(verdict_for(true, true, true), Gate::RefuseBlank);
    }

    /// A platform that drew this canvas before and now draws nothing has
    /// regressed, which is the whole point of the gate.
    #[test]
    fn a_canvas_that_stopped_drawing_fails_against_its_reference() {
        assert_eq!(verdict_for(true, false, true), Gate::RefuseBlank);
    }

    /// ...but a platform that has never drawn it, and cannot, is advised
    /// rather than left permanently red.
    #[test]
    fn a_blank_capture_with_no_reference_is_advisory() {
        assert_eq!(verdict_for(true, false, false), Gate::AdviseBlank);
    }

    #[test]
    fn a_drawn_canvas_compares_when_a_reference_exists_and_advises_when_it_does_not() {
        assert_eq!(verdict_for(false, false, true), Gate::Compare);
        assert_eq!(verdict_for(false, false, false), Gate::Advise);
        assert_eq!(verdict_for(false, true, true), Gate::Record);
    }

    #[test]
    fn references_are_keyed_by_platform() {
        let path = reference_path(Path::new("refs"), "light-cycle");

        assert_eq!(
            path.file_name().unwrap().to_str().unwrap(),
            format!("light-cycle.{}.ink", platform_key())
        );
    }
}
