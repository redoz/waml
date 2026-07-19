//! The WAML wordmark logo, drawn as an anti-aliased SDF instead of via `DrawSvg`.
//! `DrawSvg` tessellates the vector paths on the CPU with no GPU-side AA, so at
//! wordmark size the diagonal edges stair-stepped badly. Here the 6 zigzag bars
//! are rasterized analytically in the shader (each a convex quad = max of its 4
//! outward half-plane distances, centroid-oriented so winding doesn't matter),
//! giving smooth edges at any size.
//!
//! The bars are painted in fold order -- the thin up-strokes (2,4,6) first, then
//! the thick down-strokes (1,3,5) over them -- so the W reads as a folded ribbon
//! and the overlapping seams stay clean (no outlines needed).
//!
//! Geometry is normalized (0..1) against the tight content box of `waml.dxf`
//! (aspect ~1.749, y top->bottom to match `self.pos`); the shader scales it to
//! whatever draw rect the caller supplies. `mod.draw.LogoMark` is a `DrawQuad`
//! subclass (so it can attach to a `View`/`SolidView` `draw_bg`, a `DrawQuad`),
//! shared by `app.rs` (top-bar wordmark) and `start_screen.rs` (launcher card).
//! Everything is inlined in `pixel` -- custom shader helper fns added on a DSL
//! subclass don't get compiled into the shader (they silently no-op), so the
//! per-edge math is spelled out. Recolor via the `k1..k6` constants.

use makepad_widgets::*;

script_mod! {
    use mod.prelude.widgets_internal.*
    use mod.draw

    mod.draw.LogoMark = mod.draw.DrawQuad{
        pixel: fn() {
            let r = self.rect_size
            let p = self.pos * r
            let aa = 1.2
            // Dilate every bar ~0.75px so adjacent bars overlap instead of
            // meeting edge-to-edge. Edge-to-edge, both bars' AA falloff dips
            // coverage below 1 at the shared seam, so the background bleeds a
            // bright hairline through the gap; the overlap lets the top (fold-
            // order) bar cover the seam cleanly.
            let grow = 0.75

            // Per-segment fill colors (recolor here). Greyscale ramp: value is
            // the luminance of each stroke, left-to-right (seg1..seg6).
            // 3-level ramp, pattern 1,3,3,2,2,1 (1 lightest, 3 darkest).
            let k1 = vec3(0.40, 0.40, 0.40)
            let k2 = vec3(0.15, 0.15, 0.15)
            let k3 = vec3(0.15, 0.15, 0.15)
            let k4 = vec3(0.28, 0.28, 0.28)
            let k5 = vec3(0.28, 0.28, 0.28)
            let k6 = vec3(0.40, 0.40, 0.40)

            // ---- seg2 (thin up-stroke) ----
            let s2a = vec2(0.3142, 1.0000) * r
            let s2b = vec2(0.3988, 0.5445) * r
            let s2c = vec2(0.3312, 0.1569) * r
            let s2d = vec2(0.2465, 0.6125) * r
            let m2 = (s2a + s2b + s2c + s2d) * 0.25
            let n2ab = normalize(vec2((s2b - s2a).y, 0.0 - (s2b - s2a).x))
            let n2bc = normalize(vec2((s2c - s2b).y, 0.0 - (s2c - s2b).x))
            let n2cd = normalize(vec2((s2d - s2c).y, 0.0 - (s2d - s2c).x))
            let n2da = normalize(vec2((s2a - s2d).y, 0.0 - (s2a - s2d).x))
            let d2ab = dot(p - s2a, n2ab * (0.0 - sign(dot(n2ab, m2 - s2a))))
            let d2bc = dot(p - s2b, n2bc * (0.0 - sign(dot(n2bc, m2 - s2b))))
            let d2cd = dot(p - s2c, n2cd * (0.0 - sign(dot(n2cd, m2 - s2c))))
            let d2da = dot(p - s2d, n2da * (0.0 - sign(dot(n2da, m2 - s2d))))
            let dq2 = max(max(d2ab, d2bc), max(d2cd, d2da))

            // ---- seg4 (thin up-stroke) ----
            let s4a = vec2(0.6180, 1.0000) * r
            let s4b = vec2(0.6832, 0.6490) * r
            let s4c = vec2(0.6155, 0.2615) * r
            let s4d = vec2(0.5503, 0.6125) * r
            let m4 = (s4a + s4b + s4c + s4d) * 0.25
            let n4ab = normalize(vec2((s4b - s4a).y, 0.0 - (s4b - s4a).x))
            let n4bc = normalize(vec2((s4c - s4b).y, 0.0 - (s4c - s4b).x))
            let n4cd = normalize(vec2((s4d - s4c).y, 0.0 - (s4d - s4c).x))
            let n4da = normalize(vec2((s4a - s4d).y, 0.0 - (s4a - s4d).x))
            let d4ab = dot(p - s4a, n4ab * (0.0 - sign(dot(n4ab, m4 - s4a))))
            let d4bc = dot(p - s4b, n4bc * (0.0 - sign(dot(n4bc, m4 - s4b))))
            let d4cd = dot(p - s4c, n4cd * (0.0 - sign(dot(n4cd, m4 - s4c))))
            let d4da = dot(p - s4d, n4da * (0.0 - sign(dot(n4da, m4 - s4d))))
            let dq4 = max(max(d4ab, d4bc), max(d4cd, d4da))

            // ---- seg6 (thin up-stroke, rightmost) ----
            let s6a = vec2(0.8840, 1.0000) * r
            let s6b = vec2(1.0000, 0.3758) * r
            let s6c = vec2(0.8604, 0.3758) * r
            let s6d = vec2(0.8164, 0.6125) * r
            let m6 = (s6a + s6b + s6c + s6d) * 0.25
            let n6ab = normalize(vec2((s6b - s6a).y, 0.0 - (s6b - s6a).x))
            let n6bc = normalize(vec2((s6c - s6b).y, 0.0 - (s6c - s6b).x))
            let n6cd = normalize(vec2((s6d - s6c).y, 0.0 - (s6d - s6c).x))
            let n6da = normalize(vec2((s6a - s6d).y, 0.0 - (s6a - s6d).x))
            let d6ab = dot(p - s6a, n6ab * (0.0 - sign(dot(n6ab, m6 - s6a))))
            let d6bc = dot(p - s6b, n6bc * (0.0 - sign(dot(n6bc, m6 - s6b))))
            let d6cd = dot(p - s6c, n6cd * (0.0 - sign(dot(n6cd, m6 - s6c))))
            let d6da = dot(p - s6d, n6da * (0.0 - sign(dot(n6da, m6 - s6d))))
            let dq6 = max(max(d6ab, d6bc), max(d6cd, d6da))

            // ---- seg1 (thick down-stroke, leftmost) ----
            let s1a = vec2(0.0000, 0.0000) * r
            let s1b = vec2(0.1396, 0.0000) * r
            let s1c = vec2(0.3142, 1.0000) * r
            let s1d = vec2(0.1746, 1.0000) * r
            let m1 = (s1a + s1b + s1c + s1d) * 0.25
            let n1ab = normalize(vec2((s1b - s1a).y, 0.0 - (s1b - s1a).x))
            let n1bc = normalize(vec2((s1c - s1b).y, 0.0 - (s1c - s1b).x))
            let n1cd = normalize(vec2((s1d - s1c).y, 0.0 - (s1d - s1c).x))
            let n1da = normalize(vec2((s1a - s1d).y, 0.0 - (s1a - s1d).x))
            let d1ab = dot(p - s1a, n1ab * (0.0 - sign(dot(n1ab, m1 - s1a))))
            let d1bc = dot(p - s1b, n1bc * (0.0 - sign(dot(n1bc, m1 - s1b))))
            let d1cd = dot(p - s1c, n1cd * (0.0 - sign(dot(n1cd, m1 - s1c))))
            let d1da = dot(p - s1d, n1da * (0.0 - sign(dot(n1da, m1 - s1d))))
            let dq1 = max(max(d1ab, d1bc), max(d1cd, d1da))

            // ---- seg3 (thick down-stroke) ----
            let s3a = vec2(0.3312, 0.1569) * r
            let s3b = vec2(0.4708, 0.1569) * r
            let s3c = vec2(0.6180, 1.0000) * r
            let s3d = vec2(0.4783, 1.0000) * r
            let m3 = (s3a + s3b + s3c + s3d) * 0.25
            let n3ab = normalize(vec2((s3b - s3a).y, 0.0 - (s3b - s3a).x))
            let n3bc = normalize(vec2((s3c - s3b).y, 0.0 - (s3c - s3b).x))
            let n3cd = normalize(vec2((s3d - s3c).y, 0.0 - (s3d - s3c).x))
            let n3da = normalize(vec2((s3a - s3d).y, 0.0 - (s3a - s3d).x))
            let d3ab = dot(p - s3a, n3ab * (0.0 - sign(dot(n3ab, m3 - s3a))))
            let d3bc = dot(p - s3b, n3bc * (0.0 - sign(dot(n3bc, m3 - s3b))))
            let d3cd = dot(p - s3c, n3cd * (0.0 - sign(dot(n3cd, m3 - s3c))))
            let d3da = dot(p - s3d, n3da * (0.0 - sign(dot(n3da, m3 - s3d))))
            let dq3 = max(max(d3ab, d3bc), max(d3cd, d3da))

            // ---- seg5 (thick down-stroke) ----
            let s5a = vec2(0.6155, 0.2615) * r
            let s5b = vec2(0.7551, 0.2615) * r
            let s5c = vec2(0.8840, 1.0000) * r
            let s5d = vec2(0.7444, 1.0000) * r
            let m5 = (s5a + s5b + s5c + s5d) * 0.25
            let n5ab = normalize(vec2((s5b - s5a).y, 0.0 - (s5b - s5a).x))
            let n5bc = normalize(vec2((s5c - s5b).y, 0.0 - (s5c - s5b).x))
            let n5cd = normalize(vec2((s5d - s5c).y, 0.0 - (s5d - s5c).x))
            let n5da = normalize(vec2((s5a - s5d).y, 0.0 - (s5a - s5d).x))
            let d5ab = dot(p - s5a, n5ab * (0.0 - sign(dot(n5ab, m5 - s5a))))
            let d5bc = dot(p - s5b, n5bc * (0.0 - sign(dot(n5bc, m5 - s5b))))
            let d5cd = dot(p - s5c, n5cd * (0.0 - sign(dot(n5cd, m5 - s5c))))
            let d5da = dot(p - s5d, n5da * (0.0 - sign(dot(n5da, m5 - s5d))))
            let dq5 = max(max(d5ab, d5bc), max(d5cd, d5da))

            // Composite in fold order: 2,4,6 then 1,3,5 over the top.
            // Premultiplied "over" per bar (opaque color, aa-px soft edge).
            let c2 = clamp(0.5 - (dq2 - grow) / aa, 0.0, 1.0)
            let acc2 = vec4(k2, 1.0) * c2
            let c4 = clamp(0.5 - (dq4 - grow) / aa, 0.0, 1.0)
            let acc4 = vec4(k4, 1.0) * c4 + acc2 * (1.0 - c4)
            let c6 = clamp(0.5 - (dq6 - grow) / aa, 0.0, 1.0)
            let acc6 = vec4(k6, 1.0) * c6 + acc4 * (1.0 - c6)
            let c1 = clamp(0.5 - (dq1 - grow) / aa, 0.0, 1.0)
            let acc1 = vec4(k1, 1.0) * c1 + acc6 * (1.0 - c1)
            let c3 = clamp(0.5 - (dq3 - grow) / aa, 0.0, 1.0)
            let acc3 = vec4(k3, 1.0) * c3 + acc1 * (1.0 - c3)
            let c5 = clamp(0.5 - (dq5 - grow) / aa, 0.0, 1.0)
            let acc5 = vec4(k5, 1.0) * c5 + acc3 * (1.0 - c5)

            return acc5
        }
    }
}
