//! The rendering gate's comparator: an INK MASK, not a pixel image.
//!
//! # Why not a pixel diff
//!
//! The obvious gate -- screenshot, byte-compare, allow a small changed-pixel
//! ratio -- was already built for this repo once
//! (`scripts/check-use-case-diagram-screenshots.ps1`) and was never wired to
//! anything. The reason is structural, not effort: between the thing under
//! test (where a connector runs, how thick a stroke is) and the bytes being
//! compared sit at least four sources of variance this project has already
//! documented as real.
//!
//! * The editor strokes through SDF shaders whose antialias ramp is known to
//!   be `sqrt(2)` too soft, so every edge pixel carries a coverage value
//!   nobody has pinned.
//! * Headless makepad rasterises on the CPU, but through shaders JIT-compiled
//!   by the *host* C toolchain. Float contraction and libm differ between a
//!   Windows MSVC host and a Linux CI host, so the framebuffer is not
//!   byte-identical across machines.
//! * Text is rasterised per zoom-scaled size, so a glyph's coverage values
//!   move whenever anything upstream nudges the camera.
//! * Both canvases quantise strokes to whole device pixels at draw time
//!   (`canvas::pen`), so a stroke's coverage is a step function of dpi.
//!
//! Every one of those moves pixel VALUES. None of them moves whether a pixel
//! has ink in it. So the gate throws the values away and keeps the shape:
//! each pixel becomes a boolean -- is it far enough from the canvas
//! background to be ink -- and the comparison is between those masks.
//!
//! The trade is stated rather than hidden. An ink mask cannot see a colour
//! regression within the ink, and it cannot judge antialias quality. It sees
//! GEOMETRY: where lines run, how thick they are once quantised, where glyphs
//! sit and how big they are. That is the class the visual sign-off ledger's
//! connector and pen rows are about, and it is the class a naive pixel diff
//! drowns in noise.
//!
//! # Why the stored reference is text
//!
//! The headless rasteriser's PNG encoder does not deflate: the 2560x1680
//! screenshot it hands back is a 17 MB file. Storing a captured canvas as a
//! PNG would put ~9 MB of undiffable binary in the repo per reference, which
//! is its own reason a gate gets deleted. A mask is one bit per pixel and a
//! diagram canvas is almost all background, so run-length encoding it lands
//! in tens of kilobytes of plain text that `git diff` can at least show the
//! shape of. It also makes the stored artifact honest about what the gate
//! actually compares.
//!
//! The header carries the background colour the mask was cut at, and a
//! capture is binarised against that STORED colour rather than its own. A
//! theme regression that repainted the canvas would otherwise re-reference
//! itself and produce an identical mask; this way it lights up every pixel.
//!
//! # The tolerance, and where it comes from
//!
//! [`MAX_CHANGED_INK_RATIO`] is how much of the compared region may flip
//! before the gate calls it a regression. It is not a guess. The regression
//! this gate was built against is `90ffcf0f`, which moved a state-machine
//! back edge 8px off its midpoint and a self-loop from 16px to 24px of border
//! clearance. A 2px connector displaced 8px along a ~170px run flips roughly
//! 700 mask pixels -- the line leaves one place and arrives at another -- and
//! at dpi 2 that is four times as many. On a canvas crop of a few million
//! pixels that is a ratio around 5e-4, so the threshold sits below the
//! smallest regression on record while still absorbing a scattering of edge
//! pixels that changed which side of the ink threshold they fell on. The
//! measured margin on the real case is six times the tolerance
//! (`regression_proof`), which is the number to argue with before moving it.
//!
//! # What WILL make this fail, and is not a bug
//!
//! On one machine the headless rasteriser is deterministic -- same shader
//! source, same rustc, same dylib, same pixels -- so the expected run-to-run
//! noise is zero, not "small". The tolerance is not there to absorb drift; it
//! is there so one borderline pixel is not a failure.
//!
//! What that means in practice: **a Rust toolchain upgrade can move the
//! glyphs.** The gated canvas carries text, the shaders are compiled by the
//! host `rustc`, and a codegen change is free to shift a coverage value
//! across the ink threshold along every glyph edge. That should fail, and the
//! right response is to look at the red/green overlay -- if the only thing
//! lighting up is glyph fringe, re-record; if a connector moved, do not. A
//! gate that quietly widened its tolerance until neither case failed would be
//! back where the finding started.

use makepad_widgets::makepad_zune_png::makepad_zune_core::bit_depth::BitDepth;
use makepad_widgets::makepad_zune_png::makepad_zune_core::bytestream::ZCursor;
use makepad_widgets::makepad_zune_png::makepad_zune_core::colorspace::ColorSpace;
use makepad_widgets::makepad_zune_png::makepad_zune_core::options::EncoderOptions;
use makepad_widgets::makepad_zune_png::makepad_zune_core::result::DecodingResult;
use makepad_widgets::makepad_zune_png::{PngDecoder, PngEncoder};
use std::fmt;

/// How far a pixel's channels must sit from the background before it counts
/// as ink. Generous on purpose: an antialiased edge ramps through the whole
/// range and this gate is not trying to pin where on that ramp a pixel
/// landed.
pub(crate) const INK_THRESHOLD: u8 = 40;

/// The share of the compared region whose ink may flip before the gate fails.
/// See the module docs for the arithmetic.
pub(crate) const MAX_CHANGED_INK_RATIO: f64 = 1.0e-4;

const FORMAT_TAG: &str = "waml-ink-mask";
const FORMAT_VERSION: u32 = 1;
/// Runs are base-36 so a few hundred thousand of them stay readable and stay
/// small; the wrap keeps the file from being one enormous line that no diff
/// tool will render.
const RUN_RADIX: u32 = 36;
const RUNS_PER_LINE: usize = 24;

/// A decoded RGB image. Alpha is dropped at decode: the framebuffer the
/// headless rasteriser hands back is opaque, and carrying an always-255
/// channel only widens every buffer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Image {
    pub(crate) width: usize,
    pub(crate) height: usize,
    /// `width * height * 3` bytes, row-major, RGB.
    pub(crate) rgb: Vec<u8>,
}

#[derive(Debug)]
pub(crate) enum ImageError {
    Decode(String),
    Encode(String),
    /// A crop rect that does not fit inside the image it was taken from. A
    /// scenario bug -- a widget rect from a different frame than the
    /// screenshot -- so it names both rects.
    CropOutOfBounds {
        image: (usize, usize),
        crop: Crop,
    },
    /// The reference and the capture are different sizes. Never tolerated: a
    /// window that changed size is a regression in its own right, and
    /// comparing across one would silently compare the wrong things.
    SizeMismatch {
        reference: (usize, usize),
        actual: (usize, usize),
    },
    /// A stored reference that could not be read as one.
    Malformed(String),
}

impl fmt::Display for ImageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Decode(message) => write!(f, "failed to decode PNG: {message}"),
            Self::Encode(message) => write!(f, "failed to encode PNG: {message}"),
            Self::CropOutOfBounds { image, crop } => write!(
                f,
                "crop {}x{} at {},{} does not fit inside a {}x{} image",
                crop.width, crop.height, crop.x, crop.y, image.0, image.1
            ),
            Self::SizeMismatch { reference, actual } => write!(
                f,
                "the reference is {}x{} but the capture is {}x{}",
                reference.0, reference.1, actual.0, actual.1
            ),
            Self::Malformed(message) => write!(f, "malformed ink-mask reference: {message}"),
        }
    }
}

/// A region of a capture, in whole pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Crop {
    pub(crate) x: usize,
    pub(crate) y: usize,
    pub(crate) width: usize,
    pub(crate) height: usize,
}

impl Image {
    /// Decode a PNG the headless rasteriser wrote. Grey, RGB and RGBA all
    /// normalise to RGB; 16-bit is rejected rather than silently truncated,
    /// because nothing in this pipeline produces one and a truncation would
    /// quietly change every mask.
    pub(crate) fn decode_png(bytes: &[u8]) -> Result<Self, ImageError> {
        let mut decoder = PngDecoder::new(ZCursor::new(bytes));
        let pixels = decoder
            .decode()
            .map_err(|error| ImageError::Decode(format!("{error:?}")))?;
        let (width, height) = decoder
            .dimensions()
            .ok_or_else(|| ImageError::Decode("the PNG declares no dimensions".to_string()))?;
        let colorspace = decoder
            .colorspace()
            .ok_or_else(|| ImageError::Decode("the PNG declares no colorspace".to_string()))?;
        let DecodingResult::U8(samples) = pixels else {
            return Err(ImageError::Decode(
                "only 8-bit PNGs are supported by the rendering gate".to_string(),
            ));
        };
        Self::from_samples(width, height, colorspace.num_components(), &samples)
    }

    fn from_samples(
        width: usize,
        height: usize,
        components: usize,
        samples: &[u8],
    ) -> Result<Self, ImageError> {
        let expected = width
            .checked_mul(height)
            .and_then(|pixels| pixels.checked_mul(components))
            .ok_or_else(|| ImageError::Decode("PNG dimensions overflow".to_string()))?;
        if samples.len() < expected {
            return Err(ImageError::Decode(format!(
                "PNG declares {width}x{height}x{components} but decoded {} samples",
                samples.len()
            )));
        }
        let mut rgb = Vec::with_capacity(width * height * 3);
        for pixel in samples[..expected].chunks_exact(components) {
            match components {
                1 | 2 => rgb.extend_from_slice(&[pixel[0], pixel[0], pixel[0]]),
                3 | 4 => rgb.extend_from_slice(&pixel[..3]),
                other => {
                    return Err(ImageError::Decode(format!(
                        "unsupported PNG component count {other}"
                    )))
                }
            }
        }
        Ok(Self { width, height, rgb })
    }

    pub(crate) fn encode_png(&self) -> Result<Vec<u8>, ImageError> {
        let options =
            EncoderOptions::new(self.width, self.height, ColorSpace::RGB, BitDepth::Eight);
        let mut encoder = PngEncoder::new(&self.rgb, options);
        let mut out = Vec::new();
        encoder
            .encode(&mut out)
            .map_err(|error| ImageError::Encode(format!("{error:?}")))?;
        Ok(out)
    }

    pub(crate) fn crop(&self, crop: Crop) -> Result<Self, ImageError> {
        let fits = crop
            .x
            .checked_add(crop.width)
            .is_some_and(|right| right <= self.width)
            && crop
                .y
                .checked_add(crop.height)
                .is_some_and(|bottom| bottom <= self.height)
            && crop.width > 0
            && crop.height > 0;
        if !fits {
            return Err(ImageError::CropOutOfBounds {
                image: (self.width, self.height),
                crop,
            });
        }
        let mut rgb = Vec::with_capacity(crop.width * crop.height * 3);
        for row in crop.y..crop.y + crop.height {
            let start = (row * self.width + crop.x) * 3;
            rgb.extend_from_slice(&self.rgb[start..start + crop.width * 3]);
        }
        Ok(Self {
            width: crop.width,
            height: crop.height,
            rgb,
        })
    }

    fn pixel(&self, index: usize) -> [u8; 3] {
        let base = index * 3;
        [self.rgb[base], self.rgb[base + 1], self.rgb[base + 2]]
    }

    /// The canvas background, taken as the most common pixel value. A diagram
    /// canvas is overwhelmingly one flat colour, so the mode is the
    /// background and no caller has to name a theme colour that would then
    /// need updating whenever the palette moves.
    pub(crate) fn background(&self) -> [u8; 3] {
        let mut counts: std::collections::HashMap<[u8; 3], usize> =
            std::collections::HashMap::new();
        for index in 0..self.width * self.height {
            *counts.entry(self.pixel(index)).or_insert(0) += 1;
        }
        counts
            .into_iter()
            // Ties break on the colour value so the mode is deterministic: a
            // HashMap's iteration order is not, and a background flipping
            // between two equally common colours would flip the whole mask.
            .max_by_key(|&(colour, count)| (count, colour))
            .map(|(colour, _)| colour)
            .unwrap_or([0, 0, 0])
    }
}

/// One bit per pixel: ink, or background.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct InkMask {
    pub(crate) width: usize,
    pub(crate) height: usize,
    /// The colour the mask was cut against, carried so a capture is
    /// binarised the same way the reference was.
    pub(crate) background: [u8; 3],
    pub(crate) threshold: u8,
    bits: Vec<bool>,
}

impl InkMask {
    /// Cut a mask from an image against an explicit background. Callers pass
    /// the STORED background when comparing and the image's own sampled
    /// background when recording.
    pub(crate) fn cut(image: &Image, background: [u8; 3], threshold: u8) -> Self {
        let bits = (0..image.width * image.height)
            .map(|index| {
                let pixel = image.pixel(index);
                (0..3).any(|channel| pixel[channel].abs_diff(background[channel]) > threshold)
            })
            .collect();
        Self {
            width: image.width,
            height: image.height,
            background,
            threshold,
            bits,
        }
    }

    pub(crate) fn ink(&self) -> usize {
        self.bits.iter().filter(|bit| **bit).count()
    }

    /// The committed form: a small header plus base-36 run lengths,
    /// alternating background/ink and starting with background.
    pub(crate) fn encode(&self, platform: &str) -> String {
        let mut out = format!(
            "{FORMAT_TAG} {FORMAT_VERSION}\nplatform {platform}\nsize {} {}\nbackground {} {} {}\nthreshold {}\nink {}\nruns\n",
            self.width,
            self.height,
            self.background[0],
            self.background[1],
            self.background[2],
            self.threshold,
            self.ink()
        );
        let mut column = 0;
        for run in self.runs() {
            out.push_str(&to_radix(run));
            column += 1;
            if column == RUNS_PER_LINE {
                out.push('\n');
                column = 0;
            } else {
                out.push(' ');
            }
        }
        if column != 0 {
            out.pop();
            out.push('\n');
        }
        out
    }

    fn runs(&self) -> Vec<usize> {
        let mut runs = Vec::new();
        let mut current = false;
        let mut length = 0usize;
        for &bit in &self.bits {
            if bit == current {
                length += 1;
            } else {
                runs.push(length);
                current = bit;
                length = 1;
            }
        }
        if length > 0 {
            runs.push(length);
        }
        runs
    }

    pub(crate) fn decode(text: &str) -> Result<Self, ImageError> {
        let malformed = |message: String| ImageError::Malformed(message);
        let mut width = None;
        let mut height = None;
        let mut background = None;
        let mut threshold = None;
        let mut declared_ink = None;
        let mut lines = text.lines();
        let header = lines
            .next()
            .ok_or_else(|| malformed("the file is empty".to_string()))?;
        let mut header_parts = header.split_whitespace();
        if header_parts.next() != Some(FORMAT_TAG) {
            return Err(malformed(format!("expected a leading `{FORMAT_TAG}` tag")));
        }
        match header_parts.next().and_then(|v| v.parse::<u32>().ok()) {
            Some(FORMAT_VERSION) => {}
            other => {
                return Err(malformed(format!(
                    "unsupported format version {other:?}, expected {FORMAT_VERSION}"
                )))
            }
        }
        let mut runs_text = String::new();
        let mut in_runs = false;
        for line in lines {
            if in_runs {
                runs_text.push_str(line);
                runs_text.push(' ');
                continue;
            }
            let mut parts = line.split_whitespace();
            match parts.next() {
                Some("platform") => {}
                Some("size") => {
                    width = parts.next().and_then(|v| v.parse::<usize>().ok());
                    height = parts.next().and_then(|v| v.parse::<usize>().ok());
                }
                Some("background") => {
                    let channels: Vec<u8> = parts.filter_map(|v| v.parse::<u8>().ok()).collect();
                    if channels.len() == 3 {
                        background = Some([channels[0], channels[1], channels[2]]);
                    }
                }
                Some("threshold") => threshold = parts.next().and_then(|v| v.parse::<u8>().ok()),
                Some("ink") => declared_ink = parts.next().and_then(|v| v.parse::<usize>().ok()),
                Some("runs") => in_runs = true,
                Some(other) => return Err(malformed(format!("unknown header field `{other}`"))),
                None => {}
            }
        }
        let width = width.ok_or_else(|| malformed("no `size` field".to_string()))?;
        let height = height.ok_or_else(|| malformed("no `size` field".to_string()))?;
        let background =
            background.ok_or_else(|| malformed("no `background` field".to_string()))?;
        let threshold = threshold.ok_or_else(|| malformed("no `threshold` field".to_string()))?;
        if !in_runs {
            return Err(malformed("no `runs` section".to_string()));
        }
        let total = width
            .checked_mul(height)
            .ok_or_else(|| malformed("size overflows".to_string()))?;
        let mut bits = Vec::with_capacity(total);
        let mut current = false;
        for token in runs_text.split_whitespace() {
            let run = usize::from_str_radix(token, RUN_RADIX)
                .map_err(|_| malformed(format!("`{token}` is not a base-36 run length")))?;
            bits.resize(bits.len() + run, current);
            current = !current;
        }
        if bits.len() != total {
            return Err(malformed(format!(
                "the runs cover {} pixels but `size` declares {total}",
                bits.len()
            )));
        }
        let mask = Self {
            width,
            height,
            background,
            threshold,
            bits,
        };
        // The `ink` field is redundant with the runs on purpose: it is the
        // one number a human reads in a diff, so a file whose two halves
        // disagree is a file nobody should trust.
        if let Some(declared) = declared_ink {
            let actual = mask.ink();
            if declared != actual {
                return Err(malformed(format!(
                    "the header declares {declared} ink pixels but the runs carry {actual}"
                )));
            }
        }
        Ok(mask)
    }
}

fn to_radix(mut value: usize) -> String {
    if value == 0 {
        return "0".to_string();
    }
    let digits = b"0123456789abcdefghijklmnopqrstuvwxyz";
    let radix = RUN_RADIX as usize;
    let mut out = Vec::new();
    while value > 0 {
        out.push(digits[value % radix]);
        value /= radix;
    }
    out.reverse();
    String::from_utf8(out).expect("base-36 digits are ASCII")
}

/// The verdict of one comparison.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct InkDiff {
    /// Pixels whose ink state differs between reference and capture.
    pub(crate) changed: usize,
    /// Pixels compared.
    pub(crate) total: usize,
    /// Ink pixels in the reference, for context: a changed count is only
    /// readable next to how much ink there was.
    pub(crate) reference_ink: usize,
    /// A red/green overlay of the difference, for a human to look at.
    pub(crate) visualization: Image,
}

impl InkDiff {
    pub(crate) fn ratio(&self) -> f64 {
        if self.total == 0 {
            0.0
        } else {
            self.changed as f64 / self.total as f64
        }
    }

    pub(crate) fn within(&self, tolerance: f64) -> bool {
        self.ratio() <= tolerance
    }

    pub(crate) fn summary(&self) -> String {
        format!(
            "{} of {} pixels changed ink (ratio {:.3e}); the reference carries {} ink pixels",
            self.changed,
            self.total,
            self.ratio(),
            self.reference_ink
        )
    }
}

pub(crate) fn diff_ink(reference: &InkMask, actual: &InkMask) -> Result<InkDiff, ImageError> {
    if reference.width != actual.width || reference.height != actual.height {
        return Err(ImageError::SizeMismatch {
            reference: (reference.width, reference.height),
            actual: (actual.width, actual.height),
        });
    }
    let total = reference.bits.len();
    let mut changed = 0;
    let mut reference_ink = 0;
    let mut rgb = Vec::with_capacity(total * 3);
    for index in 0..total {
        let was = reference.bits[index];
        let now = actual.bits[index];
        if was {
            reference_ink += 1;
        }
        // Ink that vanished reads red and ink that appeared reads green, so a
        // displaced connector shows as a red ghost beside a green line and a
        // human can see which way it moved.
        let colour = match (was, now) {
            (true, false) => [220, 40, 40],
            (false, true) => [40, 200, 40],
            (true, true) => [190, 190, 190],
            (false, false) => [255, 255, 255],
        };
        if was != now {
            changed += 1;
        }
        rgb.extend_from_slice(&colour);
    }
    Ok(InkDiff {
        changed,
        total,
        reference_ink,
        visualization: Image {
            width: reference.width,
            height: reference.height,
            rgb,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::{diff_ink, Crop, Image, ImageError, InkMask, INK_THRESHOLD, MAX_CHANGED_INK_RATIO};

    const W: usize = 2000;
    const H: usize = 1572;
    const BG: [u8; 3] = [250, 249, 246];
    const INK: [u8; 3] = [32, 34, 38];

    fn canvas() -> Image {
        Image {
            width: W,
            height: H,
            rgb: BG.iter().copied().cycle().take(W * H * 3).collect(),
        }
    }

    fn set(image: &mut Image, x: usize, y: usize, colour: [u8; 3]) {
        let base = (y * image.width + x) * 3;
        image.rgb[base..base + 3].copy_from_slice(&colour);
    }

    /// A vertical connector `thickness` px wide over a 340px run -- the shape
    /// and roughly the length of the `Active -> Idle` back edge at dpi 2.
    fn connector(image: &mut Image, x: usize, thickness: usize) {
        for y in 400..740 {
            for offset in 0..thickness {
                set(image, x + offset, y, INK);
            }
        }
    }

    /// The antialias fringe a real capture carries: the pixels either side of
    /// a stroke sit somewhere on a coverage ramp, and where exactly is what
    /// the SDF softness and the JIT toolchain disagree about.
    fn fringe(image: &mut Image, x: usize, thickness: usize, level: u8) {
        for y in 400..740 {
            for edge in [x - 1, x + thickness] {
                set(image, edge, y, [level, level, level]);
            }
        }
    }

    fn cut(image: &Image) -> InkMask {
        InkMask::cut(image, BG, INK_THRESHOLD)
    }

    #[test]
    fn an_identical_capture_changes_no_ink() {
        let mut reference = canvas();
        connector(&mut reference, 900, 4);

        let diff = diff_ink(&cut(&reference), &cut(&reference)).unwrap();

        assert_eq!(diff.changed, 0);
        assert!(diff.within(MAX_CHANGED_INK_RATIO));
    }

    /// The specificity half. Both captures draw the same connector and
    /// disagree only about the antialias fringe either side of it, by a far
    /// wider margin than any real toolchain difference produces. The gate
    /// must not notice.
    #[test]
    fn an_antialias_fringe_that_moves_along_its_ramp_is_not_a_regression() {
        let mut reference = canvas();
        connector(&mut reference, 900, 4);
        fringe(&mut reference, 900, 4, 200);
        let mut actual = canvas();
        connector(&mut actual, 900, 4);
        fringe(&mut actual, 900, 4, 160);

        let diff = diff_ink(&cut(&reference), &cut(&actual)).unwrap();

        assert_eq!(diff.changed, 0, "{}", diff.summary());
        assert!(diff.within(MAX_CHANGED_INK_RATIO));
    }

    /// The sensitivity half, at the magnitude of the real regression:
    /// `90ffcf0f` moved a state-machine back edge 8px off its midpoint. At
    /// dpi 2 that is a 16px displacement of a 4px connector.
    #[test]
    fn a_connector_displaced_by_the_router_regression_fails_the_gate() {
        let mut reference = canvas();
        connector(&mut reference, 900, 4);
        let mut actual = canvas();
        connector(&mut actual, 916, 4);

        let diff = diff_ink(&cut(&reference), &cut(&actual)).unwrap();

        // The vacated columns and the newly inked ones both flip.
        assert_eq!(diff.changed, 340 * 8, "{}", diff.summary());
        assert!(
            !diff.within(MAX_CHANGED_INK_RATIO),
            "the router regression must fail the gate: {}",
            diff.summary()
        );
    }

    /// The pen ladder's own failure mode (visual sign-off ledger V1): a
    /// stroke that quantises to two device pixels instead of one. Nothing
    /// moved; the line just got heavier.
    #[test]
    fn a_stroke_that_gained_a_device_pixel_fails_the_gate() {
        let mut reference = canvas();
        connector(&mut reference, 900, 2);
        let mut actual = canvas();
        connector(&mut actual, 900, 4);

        let diff = diff_ink(&cut(&reference), &cut(&actual)).unwrap();

        assert_eq!(diff.changed, 340 * 2, "{}", diff.summary());
        assert!(!diff.within(MAX_CHANGED_INK_RATIO));
    }

    /// A handful of stray pixels stays under tolerance. The gate is not a
    /// bit-comparator wearing a hat: a few pixels landing on the other side
    /// of the ink threshold is what a borderline coverage value looks like.
    #[test]
    fn a_scatter_of_stray_pixels_stays_under_tolerance() {
        let mut reference = canvas();
        connector(&mut reference, 900, 4);
        let mut actual = reference.clone();
        for step in 0..64 {
            set(&mut actual, 100 + step * 7, 100 + step, INK);
        }

        let diff = diff_ink(&cut(&reference), &cut(&actual)).unwrap();

        assert_eq!(diff.changed, 64);
        assert!(diff.within(MAX_CHANGED_INK_RATIO), "{}", diff.summary());
    }

    /// The reason the mask carries its own background: a capture binarised
    /// against its OWN modal colour would re-reference a repainted theme and
    /// look unchanged.
    #[test]
    fn a_repainted_canvas_fails_the_gate_rather_than_re_referencing_itself() {
        let mut reference = canvas();
        connector(&mut reference, 900, 4);
        let mut actual = reference.clone();
        for index in 0..actual.rgb.len() / 3 {
            if actual.rgb[index * 3..index * 3 + 3] == BG {
                actual.rgb[index * 3..index * 3 + 3].copy_from_slice(&[20, 20, 24]);
            }
        }

        let stored = cut(&reference);
        let diff = diff_ink(
            &stored,
            &InkMask::cut(&actual, stored.background, stored.threshold),
        )
        .unwrap();

        assert!(!diff.within(MAX_CHANGED_INK_RATIO), "{}", diff.summary());
    }

    #[test]
    fn a_capture_of_a_different_size_is_a_hard_failure() {
        let reference = cut(&canvas());
        let actual = cut(&Image {
            width: W - 1,
            height: H,
            rgb: BG.iter().copied().cycle().take((W - 1) * H * 3).collect(),
        });

        let error = diff_ink(&reference, &actual).unwrap_err();

        assert!(matches!(error, ImageError::SizeMismatch { .. }), "{error}");
    }

    #[test]
    fn a_mask_round_trips_through_its_stored_form() {
        let mut original = canvas();
        connector(&mut original, 900, 4);
        let mask = cut(&original);

        let decoded = InkMask::decode(&mask.encode("windows-x86_64")).unwrap();

        assert_eq!(decoded, mask);
    }

    /// The stored form has to stay small enough that nobody minds it being
    /// in the repo -- that is half of why it is a mask and not a PNG.
    #[test]
    fn the_stored_form_of_a_diagram_sized_mask_stays_small() {
        let mut original = canvas();
        connector(&mut original, 900, 4);
        connector(&mut original, 1200, 4);

        let stored = cut(&original).encode("windows-x86_64");

        assert!(
            stored.len() < 32 * 1024,
            "a two-connector mask stored as {} bytes",
            stored.len()
        );
    }

    #[test]
    fn a_reference_whose_ink_count_disagrees_with_its_runs_is_rejected() {
        let mut original = canvas();
        connector(&mut original, 900, 4);
        let stored = cut(&original).encode("windows-x86_64");
        let corrupted = stored.replace(&format!("ink {}", cut(&original).ink()), "ink 1");

        let error = InkMask::decode(&corrupted).unwrap_err();

        assert!(matches!(error, ImageError::Malformed(_)), "{error}");
    }

    #[test]
    fn a_reference_whose_runs_do_not_cover_the_declared_size_is_rejected() {
        let stored = "waml-ink-mask 1\nplatform windows-x86_64\nsize 4 4\n\
                      background 0 0 0\nthreshold 40\nink 0\nruns\n8\n";

        let error = InkMask::decode(stored).unwrap_err();

        assert!(matches!(error, ImageError::Malformed(_)), "{error}");
    }

    #[test]
    fn png_round_trips_through_encode_and_decode() {
        let mut original = Image {
            width: 40,
            height: 30,
            rgb: BG.iter().copied().cycle().take(40 * 30 * 3).collect(),
        };
        set(&mut original, 10, 10, INK);

        let decoded = Image::decode_png(&original.encode_png().unwrap()).unwrap();

        assert_eq!(decoded, original);
    }

    #[test]
    fn crop_keeps_the_requested_window() {
        let mut original = canvas();
        set(&mut original, 12, 9, INK);

        let cropped = original
            .crop(Crop {
                x: 10,
                y: 8,
                width: 4,
                height: 3,
            })
            .unwrap();

        assert_eq!(cropped.width, 4);
        assert_eq!(cropped.height, 3);
        assert_eq!(&cropped.rgb[(4 + 2) * 3..(4 + 2) * 3 + 3], &INK);
    }

    #[test]
    fn a_crop_past_the_edge_is_rejected() {
        let error = canvas()
            .crop(Crop {
                x: W - 2,
                y: 0,
                width: 4,
                height: 4,
            })
            .unwrap_err();

        assert!(
            matches!(error, ImageError::CropOutOfBounds { .. }),
            "{error}"
        );
    }

    #[test]
    fn the_background_is_the_modal_colour() {
        let mut image = canvas();
        connector(&mut image, 900, 4);

        assert_eq!(image.background(), BG);
    }
}
