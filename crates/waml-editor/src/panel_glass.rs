//! Shared translucency + hover state machine for the floating HUD panels
//! (`tree_panel`, `inspector_panel`). Both panels float over the graph canvas
//! (app `flow: Overlay`), so at rest their interior fill is translucent and the
//! diagram shows through; hovering the panel -- or pinning it -- eases the fill
//! to fully opaque. Only the interior fill's alpha moves: the accent frame
//! stroke, text, and icons stay opaque, so the panel reads as glass, not dimmed.
//!
//! The panel's `draw_bg` shader (inlined per panel, kept in sync with
//! `frame.rs`) must expose an `opacity: uniform(1.0)` that scales its fill
//! alpha; this helper drives it via `set_uniform(live_id!(opacity), ..)`.
//!
//! Hover is tracked geometrically off raw `Event::MouseMove` containment, NOT
//! `Hit::FingerHover*`: makepad arbitrates hover to a single area per digit, and
//! a panel that derefs `View` runs `self.view.handle_event` before its own
//! hit-test, so inner child widgets (FileTree rows, ScrollBars, SelectBox) claim
//! the pointer's hover first and the panel's `view.area()` never wins
//! `FingerHoverIn`. Containment sidesteps the arbiter.

use crate::makepad_widgets::*;

/// Resting (unfocused) interior-fill alpha.
const GLASS_REST: f32 = 0.72;
/// Ease-in duration (seconds): translucent -> opaque on hover/pin. Snappy so
/// the panel solidifies promptly under the pointer.
const GLASS_SECS_IN: f64 = 0.14;
/// Ease-out duration (seconds): opaque -> translucent on leave. Longer than the
/// ease-in so the panel lingers before fading back into the canvas.
const GLASS_SECS_OUT: f64 = 0.32;

#[derive(Default)]
pub struct PanelGlass {
    /// Pointer is over the panel (geometric, see module docs).
    pub hovered: bool,
    /// Pinned: locks the fill fully opaque even when unhovered.
    pub pinned: bool,
    /// Eased interior-fill opacity, seeded on the first `draw`.
    glass: f32,
    seeded: bool,
    /// Prior frame's clock for dt-based easing (0.0 = restart on next frame).
    last_time: f64,
    frame: NextFrame,
}

impl PanelGlass {
    /// Seed the opacity on the first draw (so the panel doesn't flash from a
    /// zero-alpha interior) and push the current value to the panel's `draw_bg`
    /// `opacity` uniform. Call at the top of `draw_walk`.
    pub fn draw(&mut self, cx: &mut Cx2d, draw_bg: &mut DrawQuad) {
        if !self.seeded {
            self.glass = self.target();
            self.seeded = true;
        }
        draw_bg.set_uniform(cx, live_id!(opacity), &[self.glass]);
    }

    /// Track hover off `MouseMove` containment and ease the opacity toward its
    /// target on each armed frame. Returns `true` when the caller should redraw
    /// (the eased value changed); the new value is applied by the next `draw`.
    /// `rect` is the panel's laid-out rect (`view.area().rect(cx)`).
    pub fn handle_event(&mut self, cx: &mut Cx, event: &Event, rect: Rect) -> bool {
        if let Event::MouseMove(e) = event {
            let inside = rect.contains(e.abs);
            if inside != self.hovered {
                self.hovered = inside;
                self.arm(cx);
            }
        }
        if let Some(ne) = self.frame.is_event(event) {
            let target = self.target();
            if self.last_time == 0.0 {
                self.last_time = ne.time;
            }
            let dt = (ne.time - self.last_time).max(0.0);
            self.last_time = ne.time;
            if self.glass < target {
                let step = (dt / GLASS_SECS_IN) as f32;
                self.glass = (self.glass + step).min(target);
            } else if self.glass > target {
                let step = (dt / GLASS_SECS_OUT) as f32;
                self.glass = (self.glass - step).max(target);
            }
            if (self.glass - target).abs() > 0.0005 {
                self.frame = cx.new_next_frame();
            } else {
                self.glass = target;
                self.last_time = 0.0;
            }
            return true;
        }
        false
    }

    /// Toggle the pin (locks the fill fully opaque) and kick the ease.
    pub fn toggle_pin(&mut self, cx: &mut Cx) {
        self.pinned = !self.pinned;
        self.arm(cx);
    }

    fn target(&self) -> f32 {
        if self.hovered || self.pinned {
            1.0
        } else {
            GLASS_REST
        }
    }

    fn arm(&mut self, cx: &mut Cx) {
        self.last_time = 0.0;
        self.frame = cx.new_next_frame();
    }
}
