# Diagram Pens Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace three unrelated stroke-width vocabularies and three device-pixel snapping implementations with one `Pen` ladder plus one `mod.draw.CadPen` shader base, so every stroke on both diagram canvases is quantised, biased and antialiased the same way.

**Architecture:** A new module `crates/waml-editor/src/canvas/pen.rs` holds two halves that never mix. The **Rust half** is a pure `Pen` value type (four rungs, an `emphasized` multiplier, one `width()` accessor) plus two `Cx2d` helpers that size the quad a pen inks inside — `pen::band` and `pen::outline`. The **shader half** is `mod.draw.CadPen`, a base draw type carrying the device quantiser (`floor(w * dpi + 0.501)`), the half-pixel stroke bias, and the `sqrt(2)` antialias correction. Every canvas pen derives from `CadPen` instead of `DrawColor`. Width stops crossing the CPU/GPU boundary as a snapped number and starts crossing as a raw logical width the shader quantises itself.

**Tech Stack:** Rust, makepad (redoz fork at `C:\dev\makepad`), makepad `script_mod!` shader DSL, Sdf2d.

**Source spec:** `docs/superpowers/specs/2026-08-10-diagram-pens-design.md` (commit `347d06a4`). Read it before Task 1. It is approved — implement it, do not redesign it.

## Global Constraints

- **Work in a worktree.** Never edit the `C:\dev\waml` main checkout. Every path in this plan is relative to the worktree root; when editing, use the ABSOLUTE path inside your worktree — a main-root absolute path silently edits the shared main checkout and "passes" as baseline.
- **Full gate, every task:** `cargo test --workspace` AND `cargo clippy --workspace --all-targets --all-features -- -D warnings`. Both must be green before the commit.
- **`-D warnings` promotes `dead_code` to a hard error.** A task that removes the last production caller of a function MUST remove the function in the same task, or the gate goes red. This plan calls out every such pairing explicitly. The converse also bites: a task that ADDS a function with no production caller yet goes red too — which is why the ladder is wired into call sites in the same task it is introduced.
- **Commit style:** Conventional Commits, subject plus body. **No `Co-Authored-By` trailer and no Claude/AI attribution of any kind** — the repo owner considers it advertising.
- **The ladder, copied verbatim from the spec:**

  | rung | lpx | elements |
  | --- | --- | --- |
  | `HAIRLINE` | 1.0 | card compartment dividers, group hulls, label leaders, behavior dividers |
  | `LIGHT` | 1.5 | card border, interaction frames, lifeline stems, origin overlay |
  | `REGULAR` | 2.0 | every connector: class edges, behavior routes, messages, ghost overlay |
  | `HEAVY` | 3.0 | no resting element; the weight an emphasised connector lands on |

- **Glyph extents are NOT on the ladder** and keep their current values: `marker_size` 10.0, `nub_size` 6.0, `group_dash_period` 6.0, `ARROW_HEAD` 9.0, `X_MARK_SIZE` 10.0, dash `DASH_LEN` 5.0 / `DASH_GAP` 4.0.
- **Two migrations are intended visible changes:** class edges 3.0 -> 2.0, and lifeline stems (1.4) plus interaction frames (1.2) -> 1.5 (a doubling in device pixels at dpi 1). The behavior canvas's own divider drops 1.4 -> 1.0. Do not "fix" these back.
- **`ConstraintVeil` stays on `DrawColor`.** It is a wash-and-hatch fill, not a stroke.
- **Out of scope:** colours, dash patterns, marker shapes, glyph sizes, user-facing settings, routing, layout, camera limits, hit testing, fixing `antialias()` in the makepad fork, and the markdown/chrome surfaces (non-canvas `AccentFrame` consumers keep their zoom-driven treatment).

## Resolved risks (do not re-investigate)

- **Edge hit-testing does NOT read the drawn bar rects.** Verified while planning. The class canvas has no edge hit-testing at all (no `Subject::Edge`, no edge picking in `canvas/class/`). The behavior canvas hit-tests in WORLD space off the solved scene geometry — `canvas/behavior/hit.rs::distance_to_polyline` / `distance_to_segment` / `distance_to_rect_border` against fixed world tolerances (`EDGE_TOLERANCE` 6.0, `LIFELINE_TOLERANCE` 6.0, `FRAGMENT_BORDER_TOLERANCE` 6.0). No hit path reads a `Rect` produced by `stroke_quad`, `segment_quad`, `snap_bar_to_device` or `snap_rect`. The quad-shape change in Task 3 therefore cannot move a hit region. **Do not change `hit.rs` in any task of this plan.**
- **The "emphasis applied twice" risk is already neutralised.** `canvas/behavior/render/mod.rs::Emphasis::thickness` is the identity function by deliberate design (hover/selection are called out with colour and wash only, redoz@). Its call sites therefore collapse to a plain rung with no multiplier at all, and the method is deleted in Task 8 alongside its last caller.
- **Unresolved shader-fn calls are a HARD error, not a silent no-op.** `C:\dev\makepad\platform\script\src\shader_calls.rs:107` raises `script_err_wrong_value!("shader call target is not a function")`. That is what makes the Task 1 spike mechanically decidable. It is nevertheless verified with a negative control in Task 1, because this fork DOES silently no-op other constructs (`if` on a uniform; assigning `sdf.shape`/`sdf.dist` from a pixel fn).

## File structure

| File | Responsibility |
| --- | --- |
| `crates/waml-editor/src/canvas/pen.rs` | **NEW.** `mod.draw.CadPen` (shader half); `Pen` + rungs + `emphasized`/`width` (Rust half); `pen::band` / `pen::outline` (quad sizing); all pen unit tests and shader-source guards. |
| `crates/waml-editor/src/canvas/mod.rs` | Declares `pub(crate) mod pen;`. |
| `crates/waml-editor/src/app.rs` | Registers `crate::canvas::pen::script_mod(vm)` **before** `crate::frame::script_mod(vm)`. |
| `crates/waml-editor/src/bin/node_editor_harness.rs` | Same registration, before its own `frame::script_mod(vm)`. |
| `crates/waml-editor/src/canvas/class/widget.rs` | `EdgeLine`, `EdgeElbow`, `EdgeMarker`, `GroupBorder`, `GroupDashed` move to `CadPen`. `ConstraintVeil` does not. |
| `crates/waml-editor/src/canvas/behavior/mod.rs` | `FlowBox`, `FlowDiamond`, `FlowCircle`, `FlowTriangle`, `InteractionOpenHead`, `InteractionXMark`, `InteractionFrameBorder`, `InteractionTab` move to `CadPen`. |
| `crates/waml-editor/src/frame.rs` | `AccentFrame` moves to `CadPen`; its duplicated quantiser tests move to `pen.rs`. |
| `crates/waml-editor/src/canvas/geometry.rs` | `snap_bar_to_device` deleted; `segment_quad` demoted to a private helper of `corner_fillet`. |
| `crates/waml-editor/src/canvas/primitives.rs` | `snap_band`, `stroke_quad`, `snap_rect`, `snap_stroke_width` deleted. |
| `crates/waml-editor/src/canvas/linework.rs` | Deleted. |
| `crates/waml-editor/src/canvas/class/render/metrics.rs` | Deleted. |
| `crates/waml-editor/src/canvas/viewport.rs` | Gains `Camera::stroke_scale()`. |
| `crates/waml-editor/src/canvas/class/render/mod.rs` | Gains the relocated glyph-extent constants. |

## Judgment calls made while planning

The spec is precise about intent and deliberately loose about three mechanics. These are resolved here so no task has to improvise:

1. **`pen_setup()` becomes `pen_aa(gate) -> float`.** The spec writes `pen_setup() -> sdf.aa = sdf.aa * 1.4142136`, but `sdf` is a pixel-fn local and a shader fn cannot mutate it. `CadPen` instead exposes `pen_aa(gate: float) -> float` returning `mix(1.0, 1.4142136, gate)`, and each pen writes `sdf.aa = sdf.aa * self.pen_aa(1.0)`. The `gate` argument exists because `AccentFrame` must keep the correction gated on `screen_space` — non-canvas consumers were tuned against the soft ramp and must not change.
2. **`pen::band` is only for quads a `CadPen` inks inside; everything else uses `pen::outline`.** `band` returns the GROWN quad the spec describes (one device pixel of slack on each side of the centreline), which is only correct when a shader inks the quantised band within it — that is class edge bars and label leaders, both drawn with `EdgeLine`. Behavior routes, messages, lifeline stems, card dividers, port nubs and placement-overlay bars are drawn as flat `DrawColor` fills where the quad IS the ink; those size their rect from `pen.width()` and pass it through `pen::outline`, which is the direct successor of `snap_rect`. A flat fill needs no antialias correction, which is why the spec's pen list excludes `draw_rule` and `draw_fill`.
3. **`Pen::HEAVY` and `Pen::emphasized` get `#[allow(dead_code)]`.** The spec names both as part of the ladder's API, and states outright that `HEAVY` has "no resting element". The only 1.5x emphasis that ships is `AccentFrame`'s in-shader `mix(1.0, 1.5, self.selected)`, so neither has a production Rust call site. Under `-D warnings` they must be annotated or the gate goes red. Annotate exactly those two items with a comment citing the spec; do NOT delete them and do NOT blanket-allow the module.

---

### Task 1: Spike — prove `CadPen` can carry helper fns a derived pen calls

**Decision point.** Everything downstream is identical either way, but the shader text differs. Record the verdict in `pen.rs`'s module doc AND in the commit body so later tasks read it without re-running the spike.

**Files:**
- Create: `crates/waml-editor/src/canvas/pen.rs`
- Modify: `crates/waml-editor/src/canvas/mod.rs` (add `pub(crate) mod pen;`)
- Modify: `crates/waml-editor/src/app.rs:1373` (register before `crate::frame::script_mod(vm)`)
- Modify: `crates/waml-editor/src/bin/node_editor_harness.rs:68` (register before its `frame::script_mod(vm)`)
- Modify: `crates/waml-editor/src/canvas/class/widget.rs:46-66` (`EdgeLine`)
- Test: `crates/waml-editor/src/canvas/pen.rs` (`#[cfg(test)] mod tests`)

**Interfaces:**
- Consumes: nothing.
- Produces: `mod.draw.CadPen` (shader), `crate::canvas::pen::script_mod(vm: &mut ScriptVm)`, and the constants `pen::PEN_ROUND_BIAS: &str = "0.501"` / `pen::PEN_AA_FACTOR: &str = "1.4142136"` used only by shader-source guard tests.

- [ ] **Step 1: Read the spec**

Read `docs/superpowers/specs/2026-08-10-diagram-pens-design.md` end to end, and skim `crates/waml-editor/src/frame.rs` (the `AccentFrame` shader and the `shader_constants_match_the_padding_contract` test) — that test is the pattern every guard in this plan copies.

- [ ] **Step 2: Create `pen.rs` with the shader half, Mode A**

Create `crates/waml-editor/src/canvas/pen.rs`:

```rust
//! One pen system for every diagram stroke.
//!
//! Two halves that never mix. The SHADER half (`mod.draw.CadPen`, below) owns
//! device quantisation, the half-pixel stroke bias, and the antialias
//! correction; every canvas pen derives from it. The RUST half (`Pen`) owns the
//! weight ladder and is pure -- no `Cx`, no dpi, no zoom.
//!
//! SPIKE VERDICT: <MODE A: the shader VM carries inherited helper fns>
//!                <MODE B: it does not; each pen inlines the expressions>
//! (Task 1 replaces this line with the verdict it measured.)

use makepad_widgets::*;

/// The rounding term in the device quantiser. `0.501`, not `0.5`: `w` reaches
/// the shader through an f32 round-trip, and a one-ULP shortfall on an
/// expression meant to land exactly on an integer floors to the rung below.
/// That is the bug that made the card border oscillate between 1 and 2 device
/// pixels as the camera moved. Consumed by the shader-source guards below.
pub(in crate::canvas) const PEN_ROUND_BIAS: &str = "0.501";

/// The factor that restores the `sqrt(2)` this fork's `antialias()` drops.
pub(in crate::canvas) const PEN_AA_FACTOR: &str = "1.4142136";

script_mod! {
    use mod.prelude.widgets_internal.*

    // The one place stroke geometry is decided. A derived pen hands `pen_dev`
    // or `pen_sw` a LOGICAL width and gets device-grid geometry back; it never
    // does the arithmetic itself and never sees the camera.
    mod.draw.CadPen = mod.draw.DrawColor{
        // Logical stroke width, in lpx. The renderer pushes `Pen::width()`.
        pen_w: uniform(1.0)

        // Whole device pixels this pen resolves to. See PEN_ROUND_BIAS.
        pen_dev: fn(w: float) -> float {
            let dpi = max(1.0, self.draw_pass.dpi_factor)
            return max(1.0, floor(w * dpi + 0.501))
        }

        // Half-width plus the half device pixel that moves an SDF stroke's
        // antialias ramp off the crisp border samples. This is what `sdf.stroke`
        // wants.
        pen_sw: fn(w: float) -> float {
            let dpi = max(1.0, self.draw_pass.dpi_factor)
            return (max(1.0, floor(w * dpi + 0.501)) * 0.5 + 0.5) / dpi
        }

        // Sdf2d coverage is `clamp(-dist * aa)` with `aa = 1 /
        // length(vec2(|dFdx|, |dFdy|))` = 1/sqrt(2) for a pixel-unit quad, so
        // every stroke loses ~30% of its ink to the ramp. `gate` is 1.0 for a
        // canvas pen and `self.screen_space` for `AccentFrame`, whose non-canvas
        // consumers were tuned against the soft ramp.
        pen_aa: fn(gate: float) -> float {
            return mix(1.0, 1.4142136, gate)
        }
    }
}
```

- [ ] **Step 3: Declare and register the module**

In `crates/waml-editor/src/canvas/mod.rs`, add `pub(crate) mod pen;` to the module list (alphabetical, after `mod linework;`).

In `crates/waml-editor/src/app.rs`, immediately BEFORE the existing `crate::frame::script_mod(vm);` line, add:

```rust
        // `CadPen` is the base every canvas pen -- and `AccentFrame` itself --
        // derives from, so it must register before `frame` and before `canvas`.
        // A `mod.draw.X = mod.draw.CadPen{...}` whose base has not registered is
        // a hard script error at first draw, not a silent downgrade.
        crate::canvas::pen::script_mod(vm);
```

In `crates/waml-editor/src/bin/node_editor_harness.rs`, add the same call immediately before its `frame::script_mod(vm);` (this binary builds its own registration chain and does NOT go through `App::script_mod`; miss it and the harness dies at first draw while the app stays green).

- [ ] **Step 4: Move `EdgeLine` onto `CadPen`, Mode A**

In `crates/waml-editor/src/canvas/class/widget.rs`, change the `EdgeLine` declaration head from `mod.draw.EdgeLine = mod.draw.DrawColor{` to `mod.draw.EdgeLine = mod.draw.CadPen{`, and replace the hand-written antialias line with the base call. The pixel fn body becomes:

```
        pixel: fn() {
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            // Coverage correction from the shared base -- see `canvas::pen`.
            sdf.aa = sdf.aa * self.pen_aa(1.0)
            sdf.rect(-0.5, -0.5, self.rect_size.x + 1.0, self.rect_size.y + 1.0)
            sdf.fill(self.color)
            return sdf.result
        }
```

Keep the existing explanatory comments above the declaration; only the base type and the `sdf.aa` line change in this task. The quad-shape change lands in Task 3.

- [ ] **Step 5: Prove the probe can see a failure (negative control)**

Temporarily add a bogus call inside `EdgeLine`'s pixel fn, on the line after `sdf.aa`:

```
            let bogus = self.pen_does_not_exist(1.0)
```

Run the headless UI suite, which boots the app and draws a class diagram:

```bash
cargo test -p waml-editor --features ui-tests --test ui -- --test-threads=1
```

Expected: **RED**, or a `shader call target is not a function` diagnostic on stderr. If it is GREEN and stderr is clean, the probe is blind — record that, and the spike verdict must be deferred to the human sign-off section rather than claimed. Remove the bogus line either way before continuing.

- [ ] **Step 6: Run the probe for real**

```bash
cargo test -p waml-editor --features ui-tests --test ui -- --test-threads=1
```

Expected under **Mode A**: PASS with no shader diagnostic on stderr — inherited helper fns work.

If it FAILS with a shader-call diagnostic, switch to **Mode B**:
- Keep `mod.draw.CadPen` exactly as written, minus the three `fn` members (it still carries `pen_w`, so "every pen derives from `CadPen`" stays a meaningful guard).
- Every pen that would have called a helper inlines the expression instead. `EdgeLine`'s pixel fn under Mode B:

```
        pixel: fn() {
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            sdf.aa = sdf.aa * 1.4142136
            sdf.rect(-0.5, -0.5, self.rect_size.x + 1.0, self.rect_size.y + 1.0)
            sdf.fill(self.color)
            return sdf.result
        }
```

- Wherever a later task says `self.pen_dev(X)`, write `max(1.0, floor(X * dpi + 0.501))` with `let dpi = max(1.0, self.draw_pass.dpi_factor)` above it. Wherever it says `self.pen_sw(X)`, write `(max(1.0, floor(X * dpi + 0.501)) * 0.5 + 0.5) / dpi`. Wherever it says `self.pen_aa(g)`, write `mix(1.0, 1.4142136, g)` (or the bare `1.4142136` where `g` is the constant 1.0).
- The `PEN_ROUND_BIAS` / `PEN_AA_FACTOR` guards below become the only thing holding the duplicated expressions together, so they matter MORE under Mode B, not less.

- [ ] **Step 7: Write the guard test**

Append to `crates/waml-editor/src/canvas/pen.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    /// The shader source of one `mod.draw.X = mod.draw.Y{` block, comments
    /// stripped, from a file's `script_mod!`. Mirrors the extraction
    /// `frame.rs::shader_constants_match_the_padding_contract` uses.
    fn pen_source<'a>(src: &'a str, name: &str) -> &'a str {
        let head = format!("mod.draw.{name} = mod.draw.");
        let (_, rest) = src.split_once(&head).unwrap_or_else(|| {
            panic!("no `{head}` declaration found");
        });
        rest
    }

    /// Every canvas pen must derive from `CadPen`, not `DrawColor`. This is the
    /// regression guard: a new pen added on the old base renders soft, and
    /// nothing else in the suite would notice.
    fn assert_derives_from_cad_pen(src: &str, name: &str) {
        let head = format!("mod.draw.{name} = mod.draw.CadPen{{");
        assert!(
            src.contains(&head),
            "`{name}` must derive from CadPen, not DrawColor"
        );
    }

    #[test]
    fn cad_pen_carries_the_quantiser_and_the_coverage_correction() {
        let src = include_str!("pen.rs");
        let cad = pen_source(src, "CadPen");
        assert!(
            cad.contains(PEN_ROUND_BIAS),
            "CadPen must round with {PEN_ROUND_BIAS}, not a bare 0.5"
        );
        assert!(
            cad.contains(PEN_AA_FACTOR),
            "CadPen must restore the sqrt(2) the fork's antialias() drops"
        );
    }

    #[test]
    fn class_pens_derive_from_cad_pen() {
        let src = include_str!("class/widget.rs");
        for name in ["EdgeLine"] {
            assert_derives_from_cad_pen(src, name);
        }
    }

    /// `ConstraintVeil` is a wash-and-hatch FILL, not a stroke: it has no
    /// coverage to correct and no width to quantise, and the spec keeps it on
    /// `DrawColor` deliberately. Pin that so a future sweep does not "fix" it.
    #[test]
    fn the_constraint_veil_stays_a_plain_fill() {
        let src = include_str!("class/widget.rs");
        assert!(src.contains("mod.draw.ConstraintVeil = mod.draw.DrawColor{"));
    }
}
```

Under Mode B, `cad_pen_carries_the_quantiser_and_the_coverage_correction` will fail (the fns are gone). Replace it with a test that asserts the two constants appear in `EdgeLine`'s own source instead, and extend that test's pen list in every later pen task:

```rust
    #[test]
    fn every_pen_inlines_the_quantiser_terms() {
        let src = include_str!("class/widget.rs");
        for name in ["EdgeLine"] {
            let pen = pen_source(src, name);
            assert!(pen.contains(PEN_AA_FACTOR), "{name} lost the aa correction");
        }
    }
```

- [ ] **Step 8: Run the gate**

```bash
cargo test --workspace
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

Expected: both PASS.

- [ ] **Step 9: Record the verdict and commit**

Replace the `SPIKE VERDICT:` line in `pen.rs`'s module doc with the single mode that was measured, plus one sentence of evidence (e.g. "Mode A: the fork's shader VM resolves `self.pen_aa(...)` through `mod.draw.EdgeLine = mod.draw.CadPen{...}`; the negative control raised `shader call target is not a function`, so the probe is not blind.").

```bash
git add crates/waml-editor/src/canvas/pen.rs crates/waml-editor/src/canvas/mod.rs crates/waml-editor/src/app.rs crates/waml-editor/src/bin/node_editor_harness.rs crates/waml-editor/src/canvas/class/widget.rs
git commit -m "feat(canvas): add the CadPen shader base

Introduce mod.draw.CadPen, the single place device quantisation, the
half-pixel stroke bias and the sqrt(2) coverage correction are decided,
and move EdgeLine onto it. Registered ahead of frame and canvas because
a derived pen whose base has not registered is a hard script error at
first draw.

Records the spike verdict on helper-fn inheritance in the module doc."
```

---

### Task 2: The `Pen` ladder, and every authored width onto a rung

This is where the two intended visual changes land. It touches only NUMBERS — no quad shapes, no shaders, no deletions.

**Files:**
- Modify: `crates/waml-editor/src/canvas/pen.rs` (add the Rust half + tests)
- Modify: `crates/waml-editor/src/canvas/class/render/metrics.rs:20-33` and its tests
- Modify: `crates/waml-editor/src/canvas/linework.rs` (nothing structural; only the doc comment)
- Modify: `crates/waml-editor/src/canvas/class/render/labels.rs:15` (`LEADER_THICKNESS`)
- Modify: `crates/waml-editor/src/canvas/class/render/overlays.rs` (the two local `let thickness` in `draw_placement`)
- Modify: `crates/waml-editor/src/canvas/behavior/render/flow.rs:22` (`ROUTE_THICKNESS`)
- Modify: `crates/waml-editor/src/canvas/behavior/render/interaction.rs:62-65` (`STEM_THICKNESS`, `MESSAGE_THICKNESS`, `FRAME_THICKNESS`, `DIVIDER_THICKNESS`)

**Interfaces:**
- Consumes: nothing from Task 1 (the Rust half is independent of the shader half).
- Produces: `pub(in crate::canvas) struct Pen`, `Pen::HAIRLINE`/`LIGHT`/`REGULAR`/`HEAVY`, `Pen::emphasized(self, f64) -> Pen`, `Pen::width(self) -> f64`.

- [ ] **Step 1: Write the failing tests**

Append to `crates/waml-editor/src/canvas/pen.rs`'s `mod tests` (keep the guards from Task 1):

```rust
    use crate::canvas::viewport::{MAX_ZOOM, MIN_ZOOM};

    /// Rust mirror of `CadPen::pen_dev`, so the rounding that decides a
    /// stroke's device width is testable without a GPU. `bias` is the shader's
    /// rounding term; the shader ships `0.501` and the test below shows why a
    /// bare `0.5` is not a rounding term at all here but a coin flip.
    fn pen_dev(w: f32, dpi: f32, bias: f32) -> f32 {
        (w * dpi + bias).floor().max(1.0)
    }

    /// Every zoom the camera can reach, paired with the inverse-zoom
    /// `stroke_scale` a frame cancels the camera with. `zoom * stroke_scale` is
    /// 1.0 only to within an f32 ULP, which is the whole hazard.
    fn zoom_sweep() -> impl Iterator<Item = (f32, f32)> {
        let named = [1.7749_f64, 5.0201, 0.2828];
        let mut zoom = MIN_ZOOM;
        let mut extra = named.into_iter();
        std::iter::from_fn(move || {
            if zoom <= MAX_ZOOM {
                let z = zoom;
                zoom *= 1.01;
                return Some((z as f32, (1.0 / z) as f32));
            }
            extra.next().map(|z| (z as f32, (1.0 / z) as f32))
        })
    }

    #[test]
    fn the_ladder_climbs() {
        let rungs = [Pen::HAIRLINE, Pen::LIGHT, Pen::REGULAR, Pen::HEAVY];
        for rung in rungs {
            assert!(rung.width().is_finite() && rung.width() > 0.0);
        }
        for pair in rungs.windows(2) {
            assert!(
                pair[1].width() > pair[0].width(),
                "{} must be heavier than {}",
                pair[1].width(),
                pair[0].width()
            );
        }
        assert_eq!(Pen::HAIRLINE.width(), 1.0);
        assert_eq!(Pen::LIGHT.width(), 1.5);
        assert_eq!(Pen::REGULAR.width(), 2.0);
        assert_eq!(Pen::HEAVY.width(), 3.0);
    }

    #[test]
    fn emphasis_composes_and_names_the_heavy_rung() {
        assert_eq!(Pen::REGULAR.emphasized(1.5).width(), Pen::HEAVY.width());
        assert_eq!(Pen::REGULAR.emphasized(1.0).width(), Pen::REGULAR.width());
        assert_eq!(Pen::HAIRLINE.emphasized(1.5).width(), Pen::LIGHT.width());
    }

    /// The CAD contract: one fixed device width at EVERY zoom, for every rung.
    #[test]
    fn a_rung_holds_one_device_width_across_the_whole_zoom_range() {
        for pen in [Pen::HAIRLINE, Pen::LIGHT, Pen::REGULAR, Pen::HEAVY] {
            let w = pen.width() as f32;
            for dpi in [1.0_f32, 1.25, 1.5, 2.0] {
                let expected = (w * dpi + 0.5).floor().max(1.0);
                for (zoom, stroke_scale) in zoom_sweep() {
                    assert_eq!(
                        pen_dev(w * zoom * stroke_scale, dpi, 0.501),
                        expected,
                        "pen {w} zoom {zoom} dpi {dpi}"
                    );
                }
            }
        }
    }

    /// The bug the epsilon fixes. At the rungs and dpis where `w * dpi` lands
    /// exactly on `integer - 0.5` (LIGHT at dpi 1, HAIRLINE at dpi 1.5,
    /// REGULAR at dpi 1.25) the unbiased expression evaluates to exactly
    /// `floor(n)`, so a one-ULP shortfall floors to `n - 1` and the stroke
    /// visibly changes weight as the camera moves. Prove the naive form really
    /// is unstable, so the epsilon can never be "simplified" away.
    #[test]
    fn the_unbiased_quantiser_flips_width_somewhere_in_that_range() {
        for (pen, dpi) in [
            (Pen::LIGHT, 1.0_f32),
            (Pen::HAIRLINE, 1.5),
            (Pen::REGULAR, 1.25),
        ] {
            let w = pen.width() as f32;
            let widths: std::collections::BTreeSet<u32> = zoom_sweep()
                .map(|(zoom, ss)| pen_dev(w * zoom * ss, dpi, 0.5) as u32)
                .collect();
            assert!(
                widths.len() > 1,
                "expected the naive `+ 0.5` form to be zoom-dependent at \
                 w {w} dpi {dpi}, got {widths:?}"
            );
        }
    }

    /// The rungs land on the device widths the spec intends.
    #[test]
    fn rungs_land_on_the_intended_device_widths() {
        let table = [
            (Pen::HAIRLINE, 1.0, 2.0),
            (Pen::LIGHT, 2.0, 3.0),
            (Pen::REGULAR, 2.0, 4.0),
            (Pen::HEAVY, 3.0, 6.0),
        ];
        for (pen, at_dpi_1, at_dpi_2) in table {
            let w = pen.width() as f32;
            assert_eq!(pen_dev(w, 1.0, 0.501), at_dpi_1, "{w} at dpi 1");
            assert_eq!(pen_dev(w, 2.0, 0.501), at_dpi_2, "{w} at dpi 2");
        }
    }

    /// The card border's shader literal is a rung, spelled out. `frame.rs`
    /// cannot see `Pen` (it lives outside `crate::canvas`), so the tie is
    /// asserted from this side.
    #[test]
    fn the_card_border_is_the_light_rung() {
        let src = include_str!("../frame.rs");
        let expected = format!("floor({} * self.zoom", Pen::LIGHT.width());
        assert!(
            src.contains(&expected),
            "AccentFrame's border literal must track Pen::LIGHT ({})",
            Pen::LIGHT.width()
        );
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

```bash
cargo test -p waml-editor pen:: -- --nocapture
```

Expected: FAIL to compile with "cannot find type `Pen` in this scope".

- [ ] **Step 3: Write the Rust half**

Insert into `crates/waml-editor/src/canvas/pen.rs`, above the `script_mod!` block:

```rust
/// A stroke weight, in logical pixels, on the one ladder both canvases share.
///
/// Pure: no `Cx`, no dpi, no zoom. A rung is what a line MEANS, not how many
/// device pixels it happens to occupy -- that is the shader's decision, made
/// from `width()`, which is the only number that crosses into a shader.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(in crate::canvas) struct Pen {
    lpx: f64,
    emphasis: f64,
}

impl Pen {
    const fn new(lpx: f64) -> Pen {
        Pen {
            lpx,
            emphasis: 1.0,
        }
    }

    /// Card compartment dividers, group hulls, label leaders, behavior dividers.
    pub(in crate::canvas) const HAIRLINE: Pen = Pen::new(1.0);
    /// Card border, interaction frames, lifeline stems, origin overlay.
    pub(in crate::canvas) const LIGHT: Pen = Pen::new(1.5);
    /// Every connector: class edges, behavior routes, messages, ghost overlay.
    pub(in crate::canvas) const REGULAR: Pen = Pen::new(2.0);
    /// No resting element -- the weight an emphasised connector lands on. Named
    /// rather than left as an implicit product so the emphasised weight has a
    /// place in the ladder instead of being a number that only appears at
    /// runtime (spec, "The ladder").
    #[allow(dead_code)]
    pub(in crate::canvas) const HEAVY: Pen = Pen::new(3.0);

    /// Compose an emphasis multiplier onto a rung, as the card border already
    /// does in-shader. No production Rust call site yet: the only emphasis that
    /// ships is `AccentFrame`'s `mix(1.0, 1.5, self.selected)`, and the behavior
    /// canvas deliberately calls emphasis out with colour and wash rather than
    /// weight. The spec names it as part of the ladder's API, so it stays.
    #[allow(dead_code)]
    pub(in crate::canvas) fn emphasized(self, factor: f64) -> Pen {
        Pen {
            lpx: self.lpx,
            emphasis: self.emphasis * factor,
        }
    }

    /// The finished logical width a shader quantises. The ONLY number that
    /// crosses the CPU/GPU boundary.
    pub(in crate::canvas) fn width(self) -> f64 {
        self.lpx * self.emphasis
    }
}
```

- [ ] **Step 4: Put every authored width on a rung**

Each edit below replaces a literal with `Pen::<RUNG>.width()`. Add `use crate::canvas::pen::Pen;` to each file that needs it.

`crates/waml-editor/src/canvas/class/render/metrics.rs`, inside `for_zoom`:

```rust
            group_stroke_width: Pen::HAIRLINE.width() as f32,
            group_dash_period: 6.0,
            divider_thickness: Pen::HAIRLINE.width(),
            edge_thickness: Pen::REGULAR.width(),
            marker_size: 10.0,
            nub_size: 6.0,
```

Then update that file's own tests: `assert_eq!(metrics.edge_thickness, 3.0)` becomes `assert_eq!(metrics.edge_thickness, Pen::REGULAR.width())`, and `edge_dependents_hold_across_zoom` keeps its shape (it only compares low against high). This is the class-edge 3.0 -> 2.0 change.

`crates/waml-editor/src/canvas/class/render/labels.rs:15`:

```rust
/// Label leader legs. HAIRLINE: a leader is annotation chrome, not a connector.
const LEADER_THICKNESS: f64 = 1.0;
```
becomes a call site — delete the const and use `Pen::HAIRLINE.width()` at `labels.rs:101`:

```rust
            for bar in leader_bars(start, end, Pen::HAIRLINE.width()) {
```

(Deleting `LEADER_THICKNESS` in the same edit that removes its last use keeps `-D warnings` green.)

`crates/waml-editor/src/canvas/class/render/overlays.rs`, in `draw_placement`: the origin-overlay `let thickness = 1.5;` becomes `let thickness = Pen::LIGHT.width();` and the ghost-overlay `let thickness = 2.0;` becomes `let thickness = Pen::REGULAR.width();`.

`crates/waml-editor/src/canvas/behavior/render/flow.rs:22`:

```rust
/// Resting route stroke. REGULAR: a route is a connector, and weighs the same
/// as a class edge.
const ROUTE_THICKNESS: f64 = 2.0;
```
becomes, at the single use site (`flow.rs:173-175`):

```rust
    let thickness = draws.linework.thickness(Pen::REGULAR.width());
```
and the const is deleted. Its unit test at `flow.rs:361` (`assert_eq!(e.thickness(ROUTE_THICKNESS), ROUTE_THICKNESS)`) becomes `assert_eq!(e.thickness(Pen::REGULAR.width()), Pen::REGULAR.width())`.

`crates/waml-editor/src/canvas/behavior/render/interaction.rs:62-65` — replace the four consts with rungs at their use sites and delete the consts:

| was | value | becomes |
| --- | --- | --- |
| `STEM_THICKNESS` | 1.4 | `Pen::LIGHT.width()` (1.5) at `interaction.rs:237` |
| `MESSAGE_THICKNESS` | 2.0 | `Pen::REGULAR.width()` at `interaction.rs:403-405` |
| `FRAME_THICKNESS` | 1.2 | `Pen::LIGHT.width()` (1.5) at `interaction.rs:526-531`, `:584-589` |
| `DIVIDER_THICKNESS` | 1.4 | `Pen::HAIRLINE.width()` (1.0) at `interaction.rs:532` |

Leave `emphasis.thickness(...)` wrapping in place for now — it is the identity function and Task 8 removes it. So e.g. `interaction.rs:237` becomes:

```rust
    let thickness = draws
        .linework
        .thickness(emphasis.thickness(Pen::LIGHT.width()));
```

- [ ] **Step 5: Update the metrics doc comments**

`canvas/linework.rs`'s module doc and `class/render/metrics.rs`'s module doc both claim to own the policy. Add one line to each: "Widths now come from `canvas::pen::Pen`; this type is a shrinking shim and is deleted in the pen migration." Do not restructure them — Task 9 deletes both.

- [ ] **Step 6: Run the tests**

```bash
cargo test -p waml-editor pen:: -- --nocapture
cargo test --workspace
```

Expected: PASS. If `metrics_are_screen_space_at_supported_zooms` still asserts `3.0`, fix the assertion — the 3.0 -> 2.0 drop is the intended change, not a regression.

- [ ] **Step 7: Gate and commit**

```bash
cargo clippy --workspace --all-targets --all-features -- -D warnings
git add -A
git commit -m "feat(canvas): put every stroke on one weight ladder

Introduce Pen: four rungs, an emphasis multiplier, one width(). Every
authored stroke width on both canvases now names a rung instead of
restating a number.

Two intended visual changes land here: class edges drop 3.0 -> 2.0, and
lifeline stems (1.4) plus interaction frames (1.2) rise to 1.5, which
quantisation turns into a doubling at dpi 1. The behavior canvas's own
divider drops 1.4 -> 1.0 for the same reason in reverse."
```

---

### Task 3: `pen::band` / `pen::outline`, and class edge bars inked inside a grown quad

The quad-shape change the spec flags as the largest structural move. Hit-testing is unaffected (see "Resolved risks").

**Files:**
- Modify: `crates/waml-editor/src/canvas/pen.rs` (add the two CPU helpers + tests)
- Modify: `crates/waml-editor/src/canvas/class/widget.rs` (`EdgeLine` pixel fn + two new uniforms)
- Modify: `crates/waml-editor/src/canvas/class/render/edges.rs:36-49,72-118`
- Modify: `crates/waml-editor/src/canvas/class/render/labels.rs:101,129-136`
- Modify: `crates/waml-editor/src/canvas/geometry.rs:44-65` (delete `snap_bar_to_device`, demote `segment_quad`)

**Interfaces:**
- Consumes: `Pen` (Task 2), `mod.draw.CadPen` (Task 1).
- Produces: `pen::band(cx: &Cx2d, a: DVec2, b: DVec2, pen: Pen) -> Rect`, `pen::band_is_horizontal(a: DVec2, b: DVec2) -> bool`, `pen::outline(cx: &Cx2d, rect: Rect, pen: Pen) -> Rect`.

- [ ] **Step 1: Write the failing tests**

Append to `pen.rs`'s `mod tests`:

```rust
    use makepad_widgets::{dvec2, Rect};

    /// `band` is a CANVAS, never the ink: it must be at least one device pixel
    /// wider on each side than the widest band the shader can resolve the pen
    /// to, so a one-ULP disagreement between this f64 rounding and the shader's
    /// f32 one cannot clip the stroke.
    #[test]
    fn a_band_quad_clears_the_widest_stroke_its_pen_can_resolve_to() {
        for dpi in [1.0, 1.25, 1.5, 2.0] {
            for pen in [Pen::HAIRLINE, Pen::LIGHT, Pen::REGULAR] {
                for center in [10.0, 10.2, 10.5, 10.7] {
                    let quad = band_at(
                        dvec2(4.0, center),
                        dvec2(64.0, center),
                        pen,
                        dpi,
                    );
                    let ink = pen_dev(pen.width() as f32, dpi as f32, 0.501) as f64;
                    let thick_px = quad.size.y * dpi;
                    assert!(
                        (thick_px - thick_px.round()).abs() <= 1e-9,
                        "quad must be a whole number of device pixels"
                    );
                    assert!(
                        thick_px >= ink + 2.0 - 1e-9,
                        "quad {thick_px} must clear ink {ink} by a device pixel each side"
                    );
                    let start_px = quad.pos.y * dpi;
                    assert!((start_px - start_px.round()).abs() <= 1e-9);
                    // Still centred on the band it was asked for, to within the
                    // half device pixel the snap is allowed to move it.
                    assert!(
                        (quad.pos.y + quad.size.y * 0.5 - center).abs() <= 0.5 / dpi + 1e-9
                    );
                }
            }
        }
    }

    #[test]
    fn a_band_picks_the_axis_it_runs_longest_along() {
        assert!(band_is_horizontal(dvec2(0.0, 0.0), dvec2(40.0, 2.0)));
        assert!(!band_is_horizontal(dvec2(0.0, 0.0), dvec2(2.0, 40.0)));
        // A degenerate pair is horizontal, matching the old `stroke_quad`.
        assert!(band_is_horizontal(dvec2(5.0, 5.0), dvec2(5.0, 5.0)));
    }

    #[test]
    fn an_outline_lands_on_the_device_grid_and_keeps_room_for_its_stroke() {
        for dpi in [1.0, 1.5, 2.0] {
            let snapped = outline_at(
                Rect {
                    pos: dvec2(10.3, 20.7),
                    size: dvec2(40.4, 30.2),
                },
                Pen::LIGHT,
                dpi,
            );
            for v in [
                snapped.pos.x,
                snapped.pos.y,
                snapped.pos.x + snapped.size.x,
                snapped.pos.y + snapped.size.y,
            ] {
                assert!((v * dpi - (v * dpi).round()).abs() <= 1e-9);
            }
            // A degenerate rect cannot invert the shader's inset.
            let tiny = outline_at(
                Rect {
                    pos: dvec2(10.0, 20.0),
                    size: dvec2(0.0, 0.0),
                },
                Pen::LIGHT,
                dpi,
            );
            assert!(tiny.size.x >= Pen::LIGHT.width() * 2.0 - 1e-9);
            assert!(tiny.size.y >= Pen::LIGHT.width() * 2.0 - 1e-9);
        }
    }
```

- [ ] **Step 2: Run them to verify they fail**

```bash
cargo test -p waml-editor pen:: -- --nocapture
```

Expected: FAIL to compile — `band_at`, `outline_at`, `band_is_horizontal` are not defined.

- [ ] **Step 3: Write the two CPU helpers**

Add to `pen.rs`, below `impl Pen`. Note the dpi-free inner functions: they are what the tests call, and they keep the helpers unit-testable without a `Cx2d`.

```rust
/// Whether a stroke from `a` to `b` runs along x. Routes, edges and messages
/// are orthogonal, so the longer delta is the run and the shorter is the
/// thickness axis. A degenerate pair reads as horizontal.
pub(in crate::canvas) fn band_is_horizontal(a: DVec2, b: DVec2) -> bool {
    (a.x - b.x).abs() >= (a.y - b.y).abs()
}

/// The quad a `CadPen` inks a stroke from `a` to `b` inside.
///
/// This is a CANVAS, not the stroke. It is grown one device pixel past the
/// widest band the pen can resolve to on each side of the centreline, and the
/// shader inks the quantised band within it -- the same arrangement
/// `AccentFrame` already uses to ink a border inside a card rect. Growing here
/// is what lets width stop being a CPU concern: the shader is free to disagree
/// with this rounding by a pixel without its stroke being clipped.
///
/// Only for quads a `CadPen`-derived pen inks inside. A quad that IS the ink
/// (a flat `DrawColor` fill) wants [`outline`] instead.
pub(in crate::canvas) fn band(cx: &Cx2d, a: DVec2, b: DVec2, pen: Pen) -> Rect {
    band_at(a, b, pen, cx.current_dpi_factor())
}

fn band_at(a: DVec2, b: DVec2, pen: Pen, dpi: f64) -> Rect {
    let thick_px = (pen.width() * dpi + 0.501).floor().max(1.0) + 2.0;
    if band_is_horizontal(a, b) {
        let y0 = ((a.y + b.y) * 0.5 * dpi - thick_px * 0.5).round();
        let x0 = (a.x.min(b.x) * dpi).round();
        let x1 = (a.x.max(b.x) * dpi).round();
        Rect {
            pos: dvec2(x0 / dpi, y0 / dpi),
            size: dvec2((x1 - x0).max(1.0) / dpi, thick_px / dpi),
        }
    } else {
        let x0 = ((a.x + b.x) * 0.5 * dpi - thick_px * 0.5).round();
        let y0 = (a.y.min(b.y) * dpi).round();
        let y1 = (a.y.max(b.y) * dpi).round();
        Rect {
            pos: dvec2(x0 / dpi, y0 / dpi),
            size: dvec2(thick_px / dpi, (y1 - y0).max(1.0) / dpi),
        }
    }
}

/// The quad for a shape whose OUTLINE a pen strokes -- frames, group hulls,
/// markers -- with its edges pulled onto the device grid, and guaranteed wide
/// enough that the shader's `pen_sw` inset cannot invert it.
///
/// Also the right helper for a quad that IS the ink: a flat `DrawColor` fill
/// sized from `pen.width()` needs its edges on the grid and nothing else.
pub(in crate::canvas) fn outline(cx: &Cx2d, rect: Rect, pen: Pen) -> Rect {
    outline_at(rect, pen, cx.current_dpi_factor())
}

fn outline_at(rect: Rect, pen: Pen, dpi: f64) -> Rect {
    let floor_px = (pen.width() * 2.0 * dpi).ceil().max(1.0);
    let x0 = (rect.pos.x * dpi).round();
    let y0 = (rect.pos.y * dpi).round();
    let x1 = ((rect.pos.x + rect.size.x) * dpi)
        .round()
        .max(x0 + floor_px);
    let y1 = ((rect.pos.y + rect.size.y) * dpi)
        .round()
        .max(y0 + floor_px);
    Rect {
        pos: dvec2(x0 / dpi, y0 / dpi),
        size: dvec2((x1 - x0) / dpi, (y1 - y0) / dpi),
    }
}
```

`use makepad_widgets::*;` is already at the top of `pen.rs` and covers `Cx2d`, `DVec2`, `Rect`, `dvec2`.

- [ ] **Step 4: Teach `EdgeLine` to ink the quantised band inside its quad**

In `crates/waml-editor/src/canvas/class/widget.rs`, replace the `EdgeLine` declaration body (keeping the existing explanatory comment block above it, with one added sentence: "The quad is now a canvas, not the stroke: `pen::band` grows it a device pixel past the widest band this pen can resolve to, and the pixel fn inks the quantised band centred inside it."):

```
    mod.draw.EdgeLine = mod.draw.CadPen{
        // 1.0 -> y is the thin axis (a horizontal bar); 0.0 -> x is.
        // `pen::band_is_horizontal` decides it CPU-side, because the quad's own
        // aspect cannot: a short bar can be wider than it is long.
        thin_y: uniform(1.0)
        pixel: fn() {
            let dpi = max(1.0, self.draw_pass.dpi_factor)
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            sdf.aa = sdf.aa * self.pen_aa(1.0)
            // Whole device pixels, in lpx, on the thin axis only.
            let thick = self.pen_dev(self.pen_w) / dpi
            let tx = mix(thick, self.rect_size.x, self.thin_y)
            let ty = mix(self.rect_size.y, thick, self.thin_y)
            // Centre the band in the quad, then round the offset to a whole
            // device pixel: the quad's own origin is already on the grid, so a
            // whole-pixel offset keeps the band there too.
            let ox = floor((self.rect_size.x - tx) * 0.5 * dpi + 0.5) / dpi
            let oy = floor((self.rect_size.y - ty) * 0.5 * dpi + 0.5) / dpi
            // Grow by the half pixel Sdf2d's ramp is missing, exactly as before.
            sdf.rect(ox - 0.5, oy - 0.5, tx + 1.0, ty + 1.0)
            sdf.fill(self.color)
            return sdf.result
        }
    }
```

Under **Mode B**, substitute the inlined forms from Task 1 Step 6 for `self.pen_aa(1.0)` and `self.pen_dev(self.pen_w)`.

- [ ] **Step 5: Draw class edge bars through `pen::band`**

In `crates/waml-editor/src/canvas/class/render/edges.rs`:

- Replace the import line 2-5 with `use crate::canvas::geometry::{corner_fillet, elbow_radius, marker_geometry, ELBOW_MIN_DEVICE_PX};` and add `use crate::canvas::pen::{self, Pen};`.
- At the top of `draw_edges`, replace `let thickness = snapshot.linework.edge_thickness;` with `let pen = Pen::REGULAR;`, and push the pen's width and axis to the shader once per bar. Keep `let marker_size = snapshot.linework.marker_size;` and `let r_base = pen.width() * 2.0;`.
- Line 48 becomes `bars.push(pen::band(cx, a, b, pen));`.
- In the drawing loop (lines 72-118), the `quad` construction that re-derives the bar from `snapped_bar` keeps its shape — it is trimming the bar's ENDS against the fillet tangents, which is orthogonal to the thickness axis. Only the two `Rect` literals change to preserve the grown thickness that `pen::band` produced (they already copy `snapped_bar.size.y` / `.size.x`, so no edit is needed there). Immediately before `draws.edge.draw_abs(cx, quad);` add:

```rust
            draws
                .edge
                .set_uniform(cx, live_id!(pen_w), &[pen.width() as f32]);
            let horizontal = pen::band_is_horizontal(screen[i], screen[i + 1]);
            draws.edge.set_uniform(
                cx,
                live_id!(thin_y),
                &[if horizontal { 1.0 } else { 0.0 }],
            );
```

- `corner_fillet` reads `in_bar.size.y` / `in_bar.size.x` as `t_snap` — the bar's thickness — and now receives the GROWN quad, so its arc band would be two device pixels too fat. Fix it at the call site (`edges.rs:124-131`) by handing it thickness-corrected rects. Insert just above the `corner_fillet(` call:

```rust
            // `bars` now hold the pen's CANVAS, two device pixels wider than the
            // ink. The fillet band has to match the ink, so shrink both bars
            // back to the quantised width before handing them over.
            let ink = (pen.width() * dpi + 0.501).floor().max(1.0) / dpi;
            let deflate = |bar: Rect, horizontal: bool| -> Rect {
                if horizontal {
                    Rect {
                        pos: dvec2(bar.pos.x, bar.pos.y + (bar.size.y - ink) * 0.5),
                        size: dvec2(bar.size.x, ink),
                    }
                } else {
                    Rect {
                        pos: dvec2(bar.pos.x + (bar.size.x - ink) * 0.5, bar.pos.y),
                        size: dvec2(ink, bar.size.y),
                    }
                }
            };
            let in_bar = deflate(bars[i - 1], pen::band_is_horizontal(screen[i - 1], screen[i]));
            let out_bar = deflate(bars[i], pen::band_is_horizontal(screen[i], screen[i + 1]));
```

and pass `in_bar` / `out_bar` in place of `bars[i - 1]` / `bars[i]`.

- [ ] **Step 6: Draw label leaders through `pen::band`**

In `crates/waml-editor/src/canvas/class/render/labels.rs`:

- Drop `use crate::canvas::geometry::segment_quad;` and add `use crate::canvas::pen::{self, Pen};`.
- Rewrite `leader_bars` to take a `&Cx2d` and return each leg PAIRED WITH ITS AXIS. A leader's two legs run on different axes and either may be dropped as degenerate, so pairing inside the same `map` is the only form where a surviving leg cannot be matched to the wrong axis. Keep the existing doc comment (its "cannot be ONE quad" reasoning is unchanged) and add one sentence: "The bars are `pen::band` quads, so `EdgeLine` inks a hairline inside each leg instead of filling it edge to edge; the `bool` is `thin_y` for that leg."

```rust
fn leader_bars(cx: &Cx2d, start: DVec2, end: DVec2, pen: Pen) -> Vec<(Rect, bool)> {
    let corner = dvec2(end.x, start.y);
    let thickness = pen.width();
    [(start, corner), (corner, end)]
        .into_iter()
        .filter(|(a, b)| (a.x - b.x).abs() > thickness || (a.y - b.y).abs() > thickness)
        .map(|(a, b)| (pen::band(cx, a, b, pen), pen::band_is_horizontal(a, b)))
        .collect()
}
```

- At `labels.rs:100-103`, push the pen uniforms per leg:

```rust
            let start = edge_point_to_screen(&viewport.camera, viewport.view_rect.pos, leader[0]);
            draws
                .edge
                .set_uniform(cx, live_id!(pen_w), &[Pen::HAIRLINE.width() as f32]);
            for (bar, horizontal) in leader_bars(cx, start, end, Pen::HAIRLINE) {
                draws.edge.set_uniform(
                    cx,
                    live_id!(thin_y),
                    &[if horizontal { 1.0 } else { 0.0 }],
                );
                draws.edge.draw_abs(cx, bar);
            }
```

- [ ] **Step 7: Retire `snap_bar_to_device` and demote `segment_quad`**

In `crates/waml-editor/src/canvas/geometry.rs`:
- Delete `snap_bar_to_device` (lines 58-65) and its three tests (`snap_bar_to_device` appears at lines 256, 387, 396, 403, 413 — delete the whole tests that reference it: the ones asserting device snapping. Keep every test that does not mention it).
- Change `pub(crate) fn segment_quad` to `fn segment_quad` — its only remaining caller is `corner_fillet` inside this same file. Keep its tests (they are in the same module and can still see it).

- [ ] **Step 8: Run the gate**

```bash
cargo test --workspace
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

Expected: both PASS. If clippy reports `segment_quad` or `snap_bar_to_device` as never used, a call site was missed.

- [ ] **Step 9: Commit**

```bash
git add -A
git commit -m "refactor(canvas): ink class edges inside a pen-sized quad

pen::band emits a quad grown one device pixel past the widest band the
pen can resolve to, and EdgeLine inks the quantised band centred inside
it -- the arrangement AccentFrame already used for the card border.
Width stops crossing the CPU/GPU boundary as a snapped number.

pen::outline replaces snap_rect for shapes whose outline a pen strokes.
snap_bar_to_device is gone; segment_quad is now private to corner_fillet.

Hit testing is unaffected: no hit path reads a drawn bar rect (class
edges are not hit-tested at all, and behavior hit testing runs in world
space off the solved scene)."
```

---

### Task 4: `EdgeElbow` and `EdgeMarker` onto `CadPen`

Closes the "quantisation is partial within a single edge" gap: a crisp bar can no longer run into a soft corner and a soft arrowhead.

**Files:**
- Modify: `crates/waml-editor/src/canvas/class/widget.rs:74-152`
- Modify: `crates/waml-editor/src/canvas/class/render/edges.rs` (uniform pushes)
- Modify: `crates/waml-editor/src/canvas/pen.rs` (extend the guard list)

**Interfaces:**
- Consumes: `mod.draw.CadPen` and its `pen_dev`/`pen_sw`/`pen_aa` (Task 1); `Pen` (Task 2); `pen::band` (Task 3).
- Produces: nothing new.

- [ ] **Step 1: Move `EdgeElbow` onto `CadPen`**

Change the head to `mod.draw.EdgeElbow = mod.draw.CadPen{`, delete the `hw: uniform(1.0)` member (the shader derives it now), and rewrite the first three lines of the pixel fn:

```
        pixel: fn() {
            let dpi = max(1.0, self.draw_pass.dpi_factor)
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            sdf.aa = sdf.aa * self.pen_aa(1.0)
            // The arc band's half-width is the QUANTISED bar half-width, not the
            // raw `thickness * 0.5` the CPU used to hand over: a fractional lpx
            // half-width drew a soft 1.5px corner into a crisp 3px bar.
            let hw = self.pen_dev(self.pen_w) * 0.5 / dpi
```

and replace every remaining `self.hw` in that pixel fn with `hw`. The rest of the body (annulus, gate, bar stubs) is unchanged.

- [ ] **Step 2: Move `EdgeMarker` onto `CadPen`**

Change the head to `mod.draw.EdgeMarker = mod.draw.CadPen{`, delete `stroke_w: uniform(1.2)`, and change the pixel fn's opening and its `sdf.stroke` line:

```
        pixel: fn() {
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            // Coverage correction, as in `EdgeLine`. The half-pixel bias rides
            // the stroke width `pen_sw` returns, rather than the path, which a
            // polygon cannot be grown by.
            sdf.aa = sdf.aa * self.pen_aa(1.0)
```
...
```
            sdf.stroke(self.color, self.pen_sw(self.pen_w * 0.5))
```

`pen_w * 0.5` preserves today's relationship (the CPU pushed `thickness * 0.5` into `stroke_w`), and `pen_sw` already adds the half-pixel bias that the old `+ 0.5` did by hand.

- [ ] **Step 3: Push `pen_w` instead of `hw` and `stroke_w`**

In `crates/waml-editor/src/canvas/class/render/edges.rs`:
- Delete the `live_id!(hw)` push (around line 148-149) and add, alongside the other elbow uniforms:

```rust
                draws
                    .elbow
                    .set_uniform(cx, live_id!(pen_w), &[pen.width() as f32]);
```

- `CornerFillet::hw` is now unused by the renderer but is still read inside `geometry.rs::corner_fillet` to build the quad bounds. Keep the FIELD (it is written and read within `geometry.rs`) but stop reading it in `edges.rs`. If clippy reports the struct field as never read outside, make it `pub(crate) hw: f64` -> `hw: f64` (private to `geometry.rs`) in the same task.
- Replace the `live_id!(stroke_w)` marker push (around line 182-184) with:

```rust
                    draws
                        .marker
                        .set_uniform(cx, live_id!(pen_w), &[pen.width() as f32]);
```

- [ ] **Step 4: Extend the guard**

In `pen.rs`'s `class_pens_derive_from_cad_pen`, change the list to `["EdgeLine", "EdgeElbow", "EdgeMarker"]` (Mode B: extend `every_pen_inlines_the_quantiser_terms` with the same names).

- [ ] **Step 5: Gate and commit**

```bash
cargo test --workspace
cargo clippy --workspace --all-targets --all-features -- -D warnings
git add -A
git commit -m "fix(canvas): quantise edge corners and markers like their bars

EdgeElbow's arc band and EdgeMarker's stroke took raw fractional lpx
(thickness * 0.5) while the bar beside them was snapped to whole device
pixels, so one edge drew a crisp bar into a soft corner and a soft
arrowhead. Both now derive their geometry from CadPen's quantiser."
```

---

### Task 5: Class groups, card dividers, port nubs and placement overlays

**Files:**
- Modify: `crates/waml-editor/src/canvas/class/widget.rs:215-252` (`GroupBorder`, `GroupDashed`)
- Modify: `crates/waml-editor/src/canvas/class/render/groups.rs:85-116`
- Modify: `crates/waml-editor/src/canvas/class/render/nodes.rs:171-179,230-248`
- Modify: `crates/waml-editor/src/canvas/class/render/overlays.rs` (`draw_placement`)
- Modify: `crates/waml-editor/src/canvas/pen.rs` (extend the guard list)

**Interfaces:**
- Consumes: `Pen`, `pen::outline`, `mod.draw.CadPen`.
- Produces: nothing new.

- [ ] **Step 1: Move both group pens onto `CadPen`**

`GroupBorder` — head becomes `mod.draw.GroupBorder = mod.draw.CadPen{`, drop `stroke_w: uniform(1.0)`:

```
        pixel: fn() {
            let p = self.pos * self.rect_size
            let sdf = Sdf2d.viewport(p)
            sdf.aa = sdf.aa * self.pen_aa(1.0)
            let sw = self.pen_sw(self.pen_w)
            sdf.rect(sw, sw, self.rect_size.x - sw * 2.0, self.rect_size.y - sw * 2.0)
            sdf.stroke(self.color, sw)
            return sdf.result
        }
```

`GroupDashed` — head becomes `mod.draw.GroupDashed = mod.draw.CadPen{`, keep `dash_px: uniform(6.0)`, drop `stroke_w`, and make the same two substitutions (`sdf.aa` line added, `inset`/`self.stroke_w` -> `sw`). The dash duty mask is unchanged — keep its long comment block verbatim; it documents a signed-off visual decision.

- [ ] **Step 2: Push `pen_w` and snap through `pen::outline`**

In `crates/waml-editor/src/canvas/class/render/groups.rs`, add `use crate::canvas::pen::{self, Pen};`, replace `let framed = snap_rect(cx, screen);` with `let framed = pen::outline(cx, screen, Pen::HAIRLINE);`, and replace the two `live_id!(stroke_w)` pushes with `live_id!(pen_w)` carrying `Pen::HAIRLINE.width() as f32`. Keep the `live_id!(dash_px)` push, sourced from `snapshot.linework.group_dash_period`.

- [ ] **Step 3: Size card dividers and port nubs from the ladder**

In `crates/waml-editor/src/canvas/class/render/nodes.rs`, add `use crate::canvas::pen::{self, Pen};` and change `rule_rect` to take a `Pen` and use `pen::outline`. These are flat `DrawColor` fills where the quad IS the ink, so they take `outline`, not `band` (see "Judgment calls", item 2):

```rust
    fn rule_rect(cx: &Cx2d, screen: Rect, card_w: f64, dy: f64, zoom: f64, pen: Pen) -> Rect {
        pen::outline(
            cx,
            Rect {
                pos: dvec2(screen.pos.x, screen.pos.y + dy * zoom),
                size: dvec2(card_w, pen.width()),
            },
            pen,
        )
    }
```

Both call sites pass `Pen::HAIRLINE` instead of `linework.divider_thickness`. The port-nub `snap_rect` calls (lines 235, 242) become `pen::outline(cx, ..., Pen::HAIRLINE)`; `nub` keeps reading `linework.nub_size` (a glyph extent, not a rung).

- [ ] **Step 4: Snap the placement overlay bars**

In `crates/waml-editor/src/canvas/class/render/overlays.rs`, wrap each of the eight `Rect` literals in `draw_placement` in `pen::outline(cx, ..., pen)` where `pen` is the local `Pen::LIGHT` (origin overlay) or `Pen::REGULAR` (ghost overlay) introduced in Task 2, and replace `let thickness = Pen::LIGHT.width();` with `let pen = Pen::LIGHT;` / `let thickness = pen.width();` so the rect and the snap agree on one pen. Add `use crate::canvas::pen::{self, Pen};`.

- [ ] **Step 5: Extend the guard**

`class_pens_derive_from_cad_pen` list becomes `["EdgeLine", "EdgeElbow", "EdgeMarker", "GroupBorder", "GroupDashed"]`.

- [ ] **Step 6: Gate and commit**

```bash
cargo test --workspace
cargo clippy --workspace --all-targets --all-features -- -D warnings
git add -A
git commit -m "refactor(canvas): put class chrome on the pen

Group hulls, card compartment dividers, port nubs and the placement
overlays now take their weight from a rung and their grid alignment from
pen::outline. Both group pens derive from CadPen, so a dashed hull inks
as solidly as the edges beside it."
```

---

### Task 6: `AccentFrame`'s screen-space path onto `CadPen`

**Files:**
- Modify: `crates/waml-editor/src/frame.rs:183-293` (shader), `:309-520` (tests)
- Modify: `crates/waml-editor/src/canvas/viewport.rs` (add `Camera::stroke_scale`)
- Modify: `crates/waml-editor/src/canvas/pen.rs` (extend the guards)

**Interfaces:**
- Consumes: `mod.draw.CadPen`.
- Produces: `Camera::stroke_scale(&self) -> f32`.

- [ ] **Step 1: Add `Camera::stroke_scale`**

In `crates/waml-editor/src/canvas/viewport.rs`, inside `impl Camera`:

```rust
    /// The inverse zoom a frame multiplies its stroke by to cancel the camera,
    /// so linework holds one screen-space weight. `1/zoom` is a CAMERA fact --
    /// a pen never sees it. Lived on the deleted `*LineworkMetrics` types,
    /// which carried it as their one real field.
    pub(crate) fn stroke_scale(&self) -> f32 {
        (1.0 / self.zoom) as f32
    }
```

- [ ] **Step 2: Move `AccentFrame` onto `CadPen`**

In `crates/waml-editor/src/frame.rs`, change the head to `mod.draw.AccentFrame = mod.draw.CadPen{` and replace three lines in the pixel fn. Keep every surrounding comment; the `+ 0.001` epsilon comment must be rewritten to point at `canvas::pen` since the epsilon now lives there:

```
            let bw_dev = self.pen_dev(1.5 * self.zoom * self.stroke_scale * mix(1.0, 1.5, self.selected))
            let sw = self.pen_sw(1.5 * self.zoom * self.stroke_scale * mix(1.0, 1.5, self.selected))
```
...
```
            sdf.aa = sdf.aa * self.pen_aa(self.screen_space)
```

`AccentFrame` does NOT use `pen_w`: it computes its own width from `zoom * stroke_scale * selected` and does not accept a pushed width, and every non-canvas consumer depends on that. Leave `pen_w` at its inherited default.

Note `bw_dev` is still consumed by `let ctr = (self.bleed * dpi + bw_dev * 0.5) / dpi`, so keep both lines.

Under **Mode B**, leave `frame.rs`'s three expressions exactly as they are today (they are already the canonical form) and only add `mod.draw.CadPen` as the base plus the `pen_w` inheritance.

- [ ] **Step 3: Move the duplicated quantiser tests to `pen.rs`**

`frame.rs`'s `border_device_px`, `cad_zoom_sweep`, `a_cad_border_holds_one_device_width_across_the_whole_zoom_range` and `the_unbiased_quantiser_flips_width_somewhere_in_that_range` are now generalised by the equivalents added to `pen.rs` in Task 2. Delete all four from `frame.rs`, along with the local `MIN_ZOOM` / `MAX_ZOOM` consts if nothing else uses them.

Update `frame.rs`'s surviving `shader_constants_match_the_padding_contract`: the two assertions that quote `bw_dev`/`sw`/`sdf.aa` verbatim must be rewritten to the new expressions:

```rust
        assert!(code.contains(&format!(
            "let bw_dev = self.pen_dev({SURFACE_BORDER_PX} * self.zoom * self.stroke_scale * mix(1.0, 1.5, self.selected))"
        )));
        assert!(code.contains("let sw = self.pen_sw("));
        assert!(code.contains("sdf.aa = sdf.aa * self.pen_aa(self.screen_space)"));
```

The block-extraction in that test splits on `"\n        }\n    }\n\n    mod.draw.PanelSurface"` and on `"mod.draw.AccentFrame = mod.draw.DrawColor{"` — update the second literal to `"mod.draw.AccentFrame = mod.draw.CadPen{"` or the test panics with "AccentFrame shader".

- [ ] **Step 4: Extend the guard**

In `pen.rs`'s tests, add:

```rust
    #[test]
    fn the_accent_frame_derives_from_cad_pen() {
        let src = include_str!("../frame.rs");
        assert_derives_from_cad_pen(src, "AccentFrame");
    }
```

Task 2's `the_card_border_is_the_light_rung` asserts `floor({LIGHT} * self.zoom` against `frame.rs`. That literal is gone now — retarget it to the new form:

```rust
        let expected = format!("self.pen_dev({} * self.zoom", Pen::LIGHT.width());
```

- [ ] **Step 5: Gate and commit**

```bash
cargo test --workspace
cargo clippy --workspace --all-targets --all-features -- -D warnings
git add -A
git commit -m "refactor(frame): source the card border's quantiser from CadPen

The 0.501 rounding term, the half-pixel stroke bias and the sqrt(2)
coverage correction were written here first, by hand, in one shader.
They now come from the shared base, and the duplicated zoom-sweep tests
move to canvas::pen where they cover every rung rather than one width.

Camera::stroke_scale takes over the one real field the *LineworkMetrics
types carried."
```

---

### Task 7: The eight behavior pens onto `CadPen`

Shader-only. This is the step the spec calls the most visible: these pens never received the antialias correction, so every stroke on that canvas rasterises at roughly 0.35 / 1.0 / 0.35 coverage today — grey-fringed and about 1.7x wider than its number.

**Files:**
- Modify: `crates/waml-editor/src/canvas/behavior/mod.rs:34-182`
- Modify: `crates/waml-editor/src/canvas/pen.rs` (extend the guards)

**Interfaces:**
- Consumes: `mod.draw.CadPen`.
- Produces: nothing new. Uniform renames only, consumed in Task 8.

- [ ] **Step 1: The four fill-only pens**

`FlowBox`, `FlowDiamond`, `FlowCircle`, `FlowTriangle` have no stroke width. For each: change `= mod.draw.DrawColor{` to `= mod.draw.CadPen{` and insert `sdf.aa = sdf.aa * self.pen_aa(1.0)` immediately after the `let sdf = Sdf2d.viewport(...)` line. Nothing else changes. Keep every existing comment.

- [ ] **Step 2: The four stroking pens**

`InteractionOpenHead`, `InteractionXMark`, `InteractionFrameBorder`, `InteractionTab`: change the base, delete the `stroke_w: uniform(...)` member, add the `sdf.aa` line, and replace `self.stroke_w` with `self.pen_sw(self.pen_w)`.

`InteractionOpenHead`:
```
        pixel: fn() {
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            sdf.aa = sdf.aa * self.pen_aa(1.0)
            sdf.move_to(self.v1.x, self.v1.y)
            sdf.line_to(self.v0.x, self.v0.y)
            sdf.line_to(self.v2.x, self.v2.y)
            sdf.stroke(self.color, self.pen_sw(self.pen_w))
            return sdf.result
        }
```

`InteractionXMark`: same shape — `sdf.aa` line added, both `sdf.stroke(self.color, self.stroke_w)` become `sdf.stroke(self.color, self.pen_sw(self.pen_w))`.

`InteractionFrameBorder`:
```
        pixel: fn() {
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            sdf.aa = sdf.aa * self.pen_aa(1.0)
            let sw = self.pen_sw(self.pen_w)
            sdf.rect(sw, sw, self.rect_size.x - sw * 2.0, self.rect_size.y - sw * 2.0)
            sdf.stroke(self.color, sw)
            return sdf.result
        }
```

`InteractionTab`: keep `border_col`, replace `let inset = self.stroke_w * 0.5` with `let inset = self.pen_sw(self.pen_w)`, replace the two `self.rect_size.* - inset` expressions unchanged (they already read `inset`), and change the final `sdf.stroke(self.border_col, self.stroke_w)` to `sdf.stroke(self.border_col, inset)`. Add the `sdf.aa` line after the `Sdf2d.viewport` line. Keep the bevel comment block verbatim.

- [ ] **Step 3: Extend the guard**

```rust
    #[test]
    fn behavior_pens_derive_from_cad_pen() {
        let src = include_str!("behavior/mod.rs");
        for name in [
            "FlowBox",
            "FlowDiamond",
            "FlowCircle",
            "FlowTriangle",
            "InteractionOpenHead",
            "InteractionXMark",
            "InteractionFrameBorder",
            "InteractionTab",
        ] {
            assert_derives_from_cad_pen(src, name);
        }
    }
```

- [ ] **Step 4: Gate and commit**

The renderers still push `live_id!(stroke_w)` at `interaction.rs:536` and `:593`; a push to a uniform the pen no longer declares is a no-op in this fork, so the build and the suite stay green while the frame temporarily draws at `pen_w`'s default of 1.0. Task 8 replaces those pushes. Note this in the commit body so a reviewer does not read it as a bug.

```bash
cargo test --workspace
cargo clippy --workspace --all-targets --all-features -- -D warnings
git add -A
git commit -m "fix(canvas): give the behavior pens the coverage correction

Every pen on the behavior canvas rasterised at roughly 0.35/1.0/0.35
coverage -- grey-fringed and about 1.7x wider than its number -- because
the sqrt(2) correction was written by hand, pen by pen, and these eight
never received it. All eight now derive from CadPen.

The four stroking pens drop their own stroke_w for CadPen's pen_w; the
renderers' stale stroke_w pushes are inert no-ops until the next commit
rewires them, so the frames draw at the default width in between."
```

---

### Task 8: Behavior widths and quads onto the pen helpers

Retires the last three snapping implementations.

**Files:**
- Modify: `crates/waml-editor/src/canvas/behavior/render/flow.rs:173-180`
- Modify: `crates/waml-editor/src/canvas/behavior/render/interaction.rs:186-253,394-443,516-603`
- Modify: `crates/waml-editor/src/canvas/behavior/render/mod.rs:103-111` (delete `Emphasis::thickness`)
- Modify: `crates/waml-editor/src/canvas/primitives.rs:48-99` (delete four functions)

**Interfaces:**
- Consumes: `Pen`, `pen::outline`, and the `pen_w` uniform from Task 7.
- Produces: nothing new.

- [ ] **Step 1: Replace `stroke_quad` with a pen-sized fill quad**

Behavior routes, messages, stems and dividers are flat `DrawColor` fills with no shader of their own: the quad IS the ink, so there is no pen to quantise a band inside a grown canvas and `pen::band` is the wrong helper (see "Judgment calls", item 2). `pen::outline` is also wrong here — its minimum-size floor is `pen.width() * 2.0` on BOTH axes, which would fatten a short dash into a square. Add a private helper to `crates/waml-editor/src/canvas/behavior/render/mod.rs` that does the exact grid snap and nothing else:

```rust
/// The quad a flat fill inks a stroke from `a` to `b` inside. Whole device
/// pixels on the thickness axis, starting on a boundary -- the invariant the
/// deleted `primitives::snap_band` carried, restated against a rung. A diagonal
/// pair is treated as whichever axis it runs longest along (routes and messages
/// are orthogonal).
pub(super) fn fill_band(cx: &Cx2d, a: DVec2, b: DVec2, pen: Pen) -> Rect {
    let dpi = cx.current_dpi_factor();
    let w = (pen.width() * dpi + 0.501).floor().max(1.0);
    if crate::canvas::pen::band_is_horizontal(a, b) {
        let y0 = ((a.y + b.y) * 0.5 * dpi - w * 0.5).round();
        let (x0, x1) = ((a.x.min(b.x) * dpi).round(), (a.x.max(b.x) * dpi).round());
        Rect {
            pos: dvec2(x0 / dpi, y0 / dpi),
            size: dvec2((x1 - x0).max(1.0) / dpi, w / dpi),
        }
    } else {
        let x0 = ((a.x + b.x) * 0.5 * dpi - w * 0.5).round();
        let (y0, y1) = ((a.y.min(b.y) * dpi).round(), (a.y.max(b.y) * dpi).round());
        Rect {
            pos: dvec2(x0 / dpi, y0 / dpi),
            size: dvec2(w / dpi, (y1 - y0).max(1.0) / dpi),
        }
    }
}
```

Add a unit test in the same module:

```rust
    #[test]
    fn a_filled_band_lands_on_whole_device_pixels() {
        // Mirrors the invariant `primitives::snap_band` used to carry: whatever
        // subpixel phase a stroke arrives at, it leaves on the grid.
        for dpi in [1.0_f64, 1.5, 2.0] {
            for center in [10.0, 10.2, 10.5, 10.7] {
                for pen in [Pen::HAIRLINE, Pen::LIGHT, Pen::REGULAR] {
                    let w = (pen.width() * dpi + 0.501).floor().max(1.0);
                    let y0 = (center * dpi - w * 0.5).round();
                    assert!(w >= 1.0, "a stroke may never round away entirely");
                    assert!((y0 / dpi + w / dpi * 0.5 - center).abs() <= 0.5 / dpi + 1e-9);
                }
            }
        }
    }
```

- [ ] **Step 2: Rewire flow routes**

In `flow.rs`, drop `stroke_quad` from the import, add `use super::fill_band;` and `use crate::canvas::pen::Pen;`, and replace lines 173-180:

```rust
    let pen = Pen::REGULAR;
    draws.fill.color = emphasis.stroke(draws.palette.line, draws.palette);
    for pair in screen.windows(2) {
        draws.fill.draw_abs(cx, fill_band(cx, pair[0], pair[1], pen));
    }
```

Update the `Emphasis` unit test at `flow.rs:361` (it currently asserts `e.thickness(...)`) — delete it; `Emphasis::thickness` goes away in Step 5.

- [ ] **Step 3: Rewire interaction segments**

In `interaction.rs`, change `draw_dashed_segment` and `draw_solid_segment` to take a `Pen` instead of `thickness: f64`, and replace `stroke_quad(cx, p0, p1, thickness)` / `stroke_quad(cx, sa, sb, thickness)` with `fill_band(cx, p0, p1, pen)` / `fill_band(cx, sa, sb, pen)`. Their five call sites (`draw_stem`, the two in `draw_message`, the two in `draw_fragment`) pass a rung directly:

| call site | pen |
| --- | --- |
| `draw_stem` (`:241`) | `Pen::LIGHT` |
| `draw_message` self-loop sides and straight run (`:414,416,425,427`) | `Pen::REGULAR` |
| `draw_fragment` operand divider (`:546`) | `Pen::HAIRLINE` |

- [ ] **Step 4: Rewire the frame border and the tab**

`draw_fragment` (`:525-537`):

```rust
    let pen = Pen::LIGHT;
    let screen = pen::outline(cx, world_rect_to_screen(viewport, fragment.rect), pen);
    draws.frame_border.color = emphasis.stroke(draws.palette.line, draws.palette);
    draws
        .frame_border
        .set_uniform(cx, live_id!(pen_w), &[pen.width() as f32]);
    draws.frame_border.draw_abs(cx, screen);
```

`draw_fragment_tab` (`:568,584-599`): `let screen = pen::outline(cx, world_rect_to_screen(viewport, fragment.rect), Pen::LIGHT);`, and the `live_id!(stroke_w)` push becomes `live_id!(pen_w)` carrying `Pen::LIGHT.width() as f32`.

`draw_stem`'s X-mark (`:250-251`) and every arrowhead push nothing new — `InteractionXMark` and `InteractionOpenHead` now read `pen_w`, so add a push before each `draw_abs`:

```rust
        draws
            .x_mark
            .set_uniform(cx, live_id!(pen_w), &[Pen::LIGHT.width() as f32]);
```
and, in `draw_arrowhead`'s open-head branch:
```rust
        draws
            .open_head
            .set_uniform(cx, live_id!(pen_w), &[Pen::REGULAR.width() as f32]);
```

- [ ] **Step 5: Delete `Emphasis::thickness` and the three snapping helpers**

`-D warnings` makes this the same commit, not a follow-up.

- `canvas/behavior/render/mod.rs`: delete `Emphasis::thickness` (lines 103-111). Every remaining `emphasis.thickness(X)` must already be gone from Steps 2-4; grep for it and unwrap any survivor to `X`. Move its explanatory comment (a CAD drawing keeps one pen weight per line role) onto the `Emphasis` enum's doc so the reasoning survives the deletion.
- `canvas/primitives.rs`: delete `snap_band`, `stroke_quad`, `snap_rect`, `snap_stroke_width` and the test `a_snapped_band_lands_on_whole_device_pixels` (its invariant now lives in the `fill_band` test). Keep `font_raster_size`, `fill_rect`, `world_rect_to_screen`, `edge_point_to_screen` and their tests.
- `canvas/behavior/render/interaction.rs`: drop `snap_rect` and `snap_stroke_width` from the import list.

- [ ] **Step 6: Gate and commit**

```bash
cargo test --workspace
cargo clippy --workspace --all-targets --all-features -- -D warnings
git add -A
git commit -m "refactor(canvas): put behavior linework on the pen helpers

Routes, messages, lifeline stems, fragment frames and operand dividers
now take their weight from a rung and their quad from one snapper.
stroke_quad, snap_band, snap_rect and snap_stroke_width are gone.

Emphasis::thickness goes with them: it was the identity function by
design (hover and selection are called out with colour and wash, never
weight), so its call sites collapse to a bare rung."
```

---

### Task 9: Delete the metrics types and relocate the glyph extents

**Files:**
- Delete: `crates/waml-editor/src/canvas/linework.rs`
- Delete: `crates/waml-editor/src/canvas/class/render/metrics.rs`
- Modify: `crates/waml-editor/src/canvas/mod.rs` (drop `mod linework;`)
- Modify: `crates/waml-editor/src/canvas/class/render/mod.rs` (drop `mod metrics;`, add the constants)
- Modify: `crates/waml-editor/src/canvas/class/widget.rs:11,708`
- Modify: `crates/waml-editor/src/canvas/class/render/{edges,groups,labels,nodes}.rs`
- Modify: `crates/waml-editor/src/canvas/behavior/render/{mod,flow,interaction}.rs`
- Modify: `crates/waml-editor/src/canvas/pen.rs` (the final exhaustive guard)

**Interfaces:**
- Consumes: `Camera::stroke_scale` (Task 6), `Pen` (Task 2).
- Produces: `render::{MARKER_SIZE, NUB_SIZE, GROUP_DASH_PERIOD}`.

- [ ] **Step 1: Relocate the glyph extents**

In `crates/waml-editor/src/canvas/class/render/mod.rs`, add:

```rust
/// Glyph extents, in lpx at zoom 1. These are LENGTHS, not stroke weights, so
/// they are not on the pen ladder (spec, "Decisions"): they keep the values the
/// deleted `LineworkMetrics` carried.
pub(super) const MARKER_SIZE: f64 = 10.0;
pub(super) const NUB_SIZE: f64 = 6.0;
pub(super) const GROUP_DASH_PERIOD: f32 = 6.0;
```

- [ ] **Step 2: Strip `LineworkMetrics` from the class render pass**

- `class/render/mod.rs`: delete `mod metrics;` and `pub(super) use metrics::LineworkMetrics;`, and delete the `linework: LineworkMetrics` field from `RenderSnapshot`.
- `class/widget.rs:11`: drop `LineworkMetrics` from the `render::{...}` import. `:708`: delete the `linework: LineworkMetrics::for_zoom(...)` line from the `RenderSnapshot` literal.
- `class/render/edges.rs`: `snapshot.linework.marker_size` -> `super::MARKER_SIZE`.
- `class/render/groups.rs`: `snapshot.linework.group_dash_period` -> `super::GROUP_DASH_PERIOD`.
- `class/render/labels.rs:84`: `snapshot.linework.marker_size` -> `super::MARKER_SIZE`.
- `class/render/nodes.rs`: `linework.nub_size` -> `super::NUB_SIZE`; delete the `linework: LineworkMetrics` parameter from `draw_card` and its call site; `:74` `snapshot.linework.frame_stroke_scale` -> `snapshot.viewport.camera.stroke_scale()`; `:79` `snapshot.linework.frame_screen_space` -> `1.0` with the comment "always 1.0 on a canvas: drops the zoom-driven stroke-alpha lift and shadow floor that only the non-canvas `AccentFrame` consumers want".
- Delete `crates/waml-editor/src/canvas/class/render/metrics.rs`.

- [ ] **Step 3: Strip `BehaviorLineworkMetrics` from the behavior render pass**

- `behavior/render/mod.rs`: drop the `use crate::canvas::linework::BehaviorLineworkMetrics;` import, the `let linework = ...` binding, and the `linework` field from both `FlowDrawResources` and `InteractionDrawResources` literals.
- `behavior/render/flow.rs`: drop the import and the struct field; `draws.linework.glyph(ARROW_HEAD)` -> `ARROW_HEAD`.
- `behavior/render/interaction.rs`: drop the import and the struct field; `draws.linework.glyph(X_MARK_SIZE)` -> `X_MARK_SIZE`; `draws.linework.glyph(ARROW_HEAD)` -> `ARROW_HEAD`; `:276` `draws.linework.frame_stroke_scale * HEAD_FRAME_WEIGHT` -> `viewport.camera.stroke_scale() * HEAD_FRAME_WEIGHT`; `:281` `draws.linework.frame_screen_space` -> `1.0` with the same comment.
- `canvas/mod.rs`: delete `mod linework;`.
- Delete `crates/waml-editor/src/canvas/linework.rs`.

- [ ] **Step 4: Add the exhaustive guard**

In `pen.rs`'s tests:

```rust
    /// Nothing in either canvas may declare a pen on the old base. This catches
    /// the regression the per-name lists above cannot: a NEW pen, added later,
    /// on `DrawColor`. `ConstraintVeil` is the one sanctioned exception (a
    /// wash-and-hatch fill, not a stroke) and is pinned by its own test.
    #[test]
    fn no_canvas_pen_is_left_on_draw_color() {
        for (label, src) in [
            ("class/widget.rs", include_str!("class/widget.rs")),
            ("behavior/mod.rs", include_str!("behavior/mod.rs")),
        ] {
            for line in src.lines() {
                let line = line.trim();
                if let Some(rest) = line.strip_prefix("mod.draw.") {
                    if rest.contains("= mod.draw.DrawColor{") {
                        assert!(
                            rest.starts_with("ConstraintVeil "),
                            "{label}: `{line}` must derive from CadPen"
                        );
                    }
                }
            }
        }
    }
```

- [ ] **Step 5: Gate and commit**

```bash
cargo test --workspace
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

Expected: PASS. Clippy is the real check here — any missed `linework` reference is a compile error, and any orphaned helper is a `dead_code` denial.

```bash
git add -A
git commit -m "refactor(canvas): delete the linework metrics types

LineworkMetrics and BehaviorLineworkMetrics are gone. Rungs are
zoom-invariant by definition, so a metrics-for-this-zoom type was
carrying exactly one real field -- frame_stroke_scale = 1/zoom -- which
is now Camera::stroke_scale, where a camera fact belongs.

Marker, nub and group-dash extents are lengths, not stroke weights, so
they stay named constants at their current values.

Adds an exhaustive guard: no canvas pen may be declared on DrawColor
except ConstraintVeil, which is a fill."
```

---

## Human visual sign-off (REQUIRED before this work is called done)

Makepad GPU drawing is not meaningfully covered by unit tests. Every task above carries a mechanical proxy — shader-source assertions plus Rust mirrors of the shader math — and **no task's completion is gated on a visual check**, so the plan runs end to end unattended. What the proxies cannot see is whether the pixels are right. These checks belong to a human, after Task 9 lands.

Use `pwsh -File scripts/capture-window.ps1 -Out shot.png -ProcessId <pid>` against a `./run.ps1` session, at low zoom, 100% and high zoom.

- [ ] **V0 — Spike verdict.** Confirm the mode recorded in `canvas/pen.rs`'s module doc matches what actually renders. If Task 1's negative control was reported blind, this is the ONLY evidence the base is wired up at all: a class diagram whose edges are missing or unstroked means Mode A silently failed and the whole tree must move to Mode B.
- [ ] **V1 — Class diagram.** A diagram with groups, compartment dividers, solid and dashed edges, arrowheads, diamonds, cardinalities and nubs (`crates/waml-editor/tests/fixtures/groups`). Every stroke holds one apparent weight across zoom; a corner and an arrowhead weigh exactly what the bar they join weighs.
- [ ] **V2 — Behavior canvas.** A sequence or activity diagram with lifelines, messages, interaction frames and tabs. The grey fringe is gone.
- [ ] **V3 — The two intended migrations.** Confirm the class-edge weight drop (3.0 -> 2.0) and the stem/frame doubling (1.4 / 1.2 -> 1.5) read as deliberate rather than as regressions.
- [ ] **V4 — Card border unchanged.** `AccentFrame`'s non-canvas consumers (panels, popups, buttons) must look exactly as they did: the `pen_aa` gate on `screen_space` is what protects them, and it is the easiest thing in this plan to get backwards.
- [ ] **V5 — `node_editor_harness`.** `cargo run --bin node_editor_harness` must still boot; it registers `frame::script_mod` on its own chain and is the one consumer that would die from a missed `pen::script_mod` call.

## Self-review notes

- **Spec coverage.** Problem §widths -> Task 2. §snapping -> Tasks 3, 5, 8. §partial quantisation -> Tasks 3, 4. §antialiasing by hand -> Tasks 1, 4, 5, 6, 7. Architecture §Rust half -> Task 2. §shader half -> Task 1. §what the CPU keeps -> Task 3. §what folds away -> Tasks 6, 9. Migration steps 1-7 -> Tasks 1, 2, 3+4, 5, 6, 7+8, 9. Testing §unit -> Task 2. §shader-source -> Tasks 1, 4, 5, 6, 7, 9. §visual -> the sign-off section. Risks §helper-fn inheritance -> Task 1. §hit testing -> resolved above. §double emphasis -> resolved above.
- **Independently shippable.** Spec steps 3-6 map to Tasks 3+4 (class edges), 5 (class chrome), 6 (`AccentFrame`), 7+8 (behavior). Each ends green and can ship alone; only Task 9's deletions require all of them.
- **Naming consistency.** `Pen::width()`, `Pen::emphasized()`, `pen::band()`, `pen::band_is_horizontal()`, `pen::outline()`, `fill_band()`, `Camera::stroke_scale()`, and the shader members `pen_w`, `thin_y`, `pen_dev`, `pen_sw`, `pen_aa` are used identically in every task above.
