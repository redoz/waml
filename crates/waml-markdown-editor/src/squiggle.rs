//! The diagnostic squiggle pen.
//!
//! An antialiased sine-wave underline, coloured per severity. The wave phase
//! is locked to absolute document x (`phase_x`, carried on the instance), so
//! the squiggle does not crawl when the viewport scrolls or text reflows --
//! and adjacent rects of one wrapped range continue a single wave seamlessly.

use makepad_widgets::*;

script_mod! {
    use mod.prelude.widgets_internal.*

    // Analytic distance from the fragment to a sine curve, stroked with
    // smoothstep. `sdf.box(.., 0)` floods the quad in this fork -- no sdf
    // helpers here at all, the distance is computed directly.
    set_type_default() do #(DrawSquiggle::script_shader(vm)){
        ..mod.draw.DrawQuad
        pixel: fn() {
            let px = self.pos * self.rect_size
            let two_pi = 6.2831853
            let period = 4.0
            let amplitude = 1.5
            let angle = (self.phase_x + px.x) * two_pi / period
            let mid = self.rect_size.y * 0.5
            let curve = mid + amplitude * sin(angle)
            let slope = amplitude * cos(angle) * two_pi / period
            let dist = abs(px.y - curve) / sqrt(1.0 + slope * slope)
            let alpha = 1.0 - smoothstep(0.5, 1.5, dist)
            return self.color * alpha
        }
    }
}

#[derive(Script, ScriptHook)]
#[repr(C)]
pub struct DrawSquiggle {
    #[deref]
    pub draw_super: DrawQuad,
    #[live]
    pub color: Vec4f,
    /// Absolute x of the quad's left edge, locking the wave phase.
    #[live(0.0)]
    pub phase_x: f32,
}
