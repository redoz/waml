//! The hairline where the window's chrome ends and its content begins: a crisp
//! 1px rule spanning the full window width at the caption bar's bottom edge.
//!
//! It used to be the doc-tab strip's top rule, reaching back over `[T]` and the
//! tree-column spacer via a runtime-measured overshoot. That worked while the
//! tab row WAS the caption's lower band, but the row now lives in
//! `center_column` (see `app.rs`), so a line spanning the whole window is no
//! longer something the strip can own: `dock_body` is an Overlay painting
//! `dock_row` -> `tree_layer` -> `inspector_layer`, and anything the strip drew
//! left of the tree column's edge was painted over by the tree panel a moment
//! later. Reordering those layers would fix the paint and invert the panels'
//! event priority over the canvas, which is not a trade worth making for a
//! hairline.
//!
//! Mounted as the caption's last row instead. The caption paints BEFORE the
//! body, so nothing can occlude it, and the rule needs no measurement at all --
//! it is simply `Fill` wide. Ownership now matches what the line means: the seam
//! belongs to the chrome, not to one of the panels it runs across.

use makepad_widgets::*;

script_mod! {
    use mod.prelude.widgets_internal.*
    use mod.atlas
    use mod.widgets.*

    mod.widgets.ChromeSeamBase = #(ChromeSeam::register_widget(vm))

    mod.widgets.ChromeSeam = set_type_default() do mod.widgets.ChromeSeamBase{
        width: Fill
        height: 1.0
        draw_edge +: { color: atlas.surface_border }
    }
}

/// Length (px) of the dissolve at the rule's right end, and how many 1px
/// segments fake it. A crisp plain quad carries one flat colour, so the taper is
/// stacked constant-alpha segments rather than a true per-pixel gradient.
const EDGE_FADE: f64 = 48.0;
const EDGE_FADE_STEPS: usize = 4;

/// Where the solid run ends, snapped to a whole device column so the fade's
/// first segment butts against it without a seam.
fn rule_x_end(right: f64) -> f64 {
    right.round()
}

/// The span the active doc tab occupies on the seam, and the colour to paint
/// there: `(x0, x1, accent)` in window coordinates. The active tab interrupts
/// the hairline -- that is how the selected document reads as continuous with
/// the canvas below it -- and since the tab cannot draw upward into the caption,
/// the seam paints the interruption itself.
type Break = (f64, f64, Vec4);

/// Split `(x0, x1)` around `brk`, returning the runs of plain rule left over.
/// Pure geometry, so the clamping cases (break off either end, break swallowing
/// the whole run) are unit-testable without a `Cx`.
fn runs_around_break(x0: f64, x1: f64, brk: Option<Break>) -> Vec<(f64, f64)> {
    let Some((bx0, bx1)) = brk.map(|(a, b, _)| (a, b)) else {
        return vec![(x0, x1)];
    };
    let bx0 = bx0.clamp(x0, x1);
    let bx1 = bx1.clamp(x0, x1);
    [(x0, bx0), (bx1, x1)]
        .into_iter()
        .filter(|(a, b)| b - a > 0.0)
        .collect()
}

#[derive(Script, ScriptHook, Widget)]
pub struct ChromeSeam {
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
    draw_edge: DrawColor,

    /// Where the active tab meets the seam (see `Break`). Runtime-driven by
    /// `App::sync_chrome_seam`; `None` draws an unbroken rule, the safe reading
    /// if that wiring is ever dropped.
    #[rust]
    tab_break: Option<Break>,
}

impl ChromeSeam {
    /// Store the active tab's span. The caller change-guards, so this only
    /// stores and repaints.
    pub fn set_tab_break(&mut self, cx: &mut Cx, brk: Option<Break>) {
        self.tab_break = brk;
        self.draw_edge.redraw(cx);
    }
}

impl Widget for ChromeSeam {
    fn handle_event(&mut self, _cx: &mut Cx, _event: &Event, _scope: &mut Scope) {
        // Inert. This sits in the caption's drag region and must never claim a
        // press -- `WindowDragQuery` names client-area rects explicitly, and a
        // 1px widget swallowing drags would be invisible to debug.
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, _scope: &mut Scope, walk: Walk) -> DrawStep {
        let rect = cx.walk_turtle(walk);
        // Snap y to a whole device row: a rule straddling two rows blurs across
        // both and stops reading as a hairline.
        let y = rect.pos.y.round();
        let x0 = rect.pos.x.round();
        let x_end = rule_x_end(rect.pos.x + rect.size.x);
        let solid_end = x_end - EDGE_FADE;
        // Read before the loop mutates it, and restored after: the field
        // persists across frames, so a fade left in place would compound.
        let base = self.draw_edge.color;

        // Solid run, split around the active tab. The fade tail is left whole:
        // it only ever covers the last `EDGE_FADE` px at the window's right
        // edge, which no tab card reaches.
        for (rx0, rx1) in runs_around_break(x0, solid_end, self.tab_break) {
            self.draw_edge.draw_abs(
                cx,
                Rect {
                    pos: dvec2(rx0, y),
                    size: dvec2(rx1 - rx0, 1.0),
                },
            );
        }
        // The break itself, in the active view's accent: the selected tab's
        // 2px accent bar begins on the row below, so painting this row to match
        // reads as one 3px flag interrupting the rule -- which is what the
        // accent-drawn-over-the-rule arrangement looked like before the rule
        // moved up into the caption.
        if let Some((bx0, bx1, accent)) = self.tab_break {
            let bx0 = bx0.clamp(x0, solid_end);
            let bx1 = bx1.clamp(x0, solid_end);
            if bx1 > bx0 {
                self.draw_edge.color = accent;
                self.draw_edge.draw_abs(
                    cx,
                    Rect {
                        pos: dvec2(bx0, y),
                        size: dvec2(bx1 - bx0, 1.0),
                    },
                );
            }
        }
        for i in 0..EDGE_FADE_STEPS {
            let step_w = EDGE_FADE / EDGE_FADE_STEPS as f64;
            let sx = solid_end + step_w * i as f64;
            // Alpha falls from ~0.875 (nearest the solid run) to ~0.125 at the edge.
            let a = 1.0 - (i as f32 + 0.5) / EDGE_FADE_STEPS as f32;
            self.draw_edge.color = Vec4 {
                w: base.w * a,
                ..base
            };
            self.draw_edge.draw_abs(
                cx,
                Rect {
                    pos: dvec2(sx, y),
                    size: dvec2(step_w, 1.0),
                },
            );
        }
        self.draw_edge.color = base;
        DrawStep::done()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The fade tiles the last `EDGE_FADE` px exactly -- no gap before the
    /// window edge, no segment overrunning it.
    #[test]
    fn fade_segments_tile_the_rule_end() {
        let right = 1_280.0;
        let x_end = rule_x_end(right);
        let solid_end = x_end - EDGE_FADE;
        let step_w = EDGE_FADE / EDGE_FADE_STEPS as f64;
        let last = solid_end + step_w * (EDGE_FADE_STEPS - 1) as f64;
        assert_eq!(last + step_w, x_end);
    }

    /// No break leaves one run; a break in the middle leaves two, tiling the
    /// span exactly with the break's own width missing.
    #[test]
    fn break_splits_the_run_without_losing_width() {
        assert_eq!(runs_around_break(0.0, 1000.0, None), vec![(0.0, 1000.0)]);
        let brk = Some((300.0, 420.0, Vec4::default()));
        assert_eq!(
            runs_around_break(0.0, 1000.0, brk),
            vec![(0.0, 300.0), (420.0, 1000.0)]
        );
    }

    /// A break at either end drops the empty run rather than emitting a
    /// zero-width quad, and one covering everything leaves no plain rule at all.
    #[test]
    fn break_at_the_edges_drops_empty_runs() {
        let c = Vec4::default();
        assert_eq!(
            runs_around_break(0.0, 1000.0, Some((0.0, 120.0, c))),
            vec![(120.0, 1000.0)]
        );
        assert_eq!(
            runs_around_break(0.0, 1000.0, Some((880.0, 1000.0, c))),
            vec![(0.0, 880.0)]
        );
        assert!(runs_around_break(0.0, 1000.0, Some((0.0, 1000.0, c))).is_empty());
    }

    /// A break reported outside the seam (a card scrolled past either end) is
    /// clamped into the span rather than producing a negative-width run.
    #[test]
    fn break_outside_the_span_is_clamped() {
        let c = Vec4::default();
        assert_eq!(
            runs_around_break(100.0, 900.0, Some((0.0, 50.0, c))),
            vec![(100.0, 900.0)]
        );
        assert_eq!(
            runs_around_break(100.0, 900.0, Some((950.0, 1200.0, c))),
            vec![(100.0, 900.0)]
        );
    }

    /// Alpha falls monotonically and never reaches full or zero: the run's own
    /// colour already covers the solid part, and a zero-alpha tail segment would
    /// just be a wasted quad.
    #[test]
    fn fade_alpha_falls_monotonically_within_bounds() {
        let alphas: Vec<f32> = (0..EDGE_FADE_STEPS)
            .map(|i| 1.0 - (i as f32 + 0.5) / EDGE_FADE_STEPS as f32)
            .collect();
        assert!(alphas.windows(2).all(|w| w[0] > w[1]));
        assert!(alphas.iter().all(|a| *a > 0.0 && *a < 1.0));
    }
}
