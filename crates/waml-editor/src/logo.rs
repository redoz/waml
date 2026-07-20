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
    use mod.atlas
    use mod.widgets.*

    mod.draw.LogoMark = mod.draw.DrawQuad{
        // Greyscale ramp stops, themed (light: dark greys; dark: light silver).
        seg_hi: uniform(atlas.logo_hi)
        seg_mid: uniform(atlas.logo_mid)
        seg_lo: uniform(atlas.logo_lo)
        // Hover shimmer: `hover` (eased 0..1) gates the effect; `time` (seconds
        // since hover-in) drives the traveling wave + breathe; `accent` is the
        // colour the fold-segments flow toward. All default to rest so the
        // start-screen card (which never sets them) draws the plain wordmark.
        accent: uniform(atlas.accent)
        hover: uniform(0.0)
        time: uniform(0.0)
        // Animation mode: 0 = hover shimmer (top-bar wordmark, the default so
        // any plain instance is unchanged); 1..6 = always-on splash letter
        // pulse variants (accent / Close Encounters / bucket-palette / and the
        // agent-authored molten / neon / electric sets). See `pixel`.
        mode: uniform(0.0)
        // Crossfade coverage scale (0..1), default 1 = solid. Only the splash
        // drives it below 1: on a logo click it draws the outgoing variant at
        // fade=1, then the incoming variant over it at fade=0->1, cross-
        // dissolving the two matched silhouettes. Every other instance leaves
        // it at rest.
        fade: uniform(1.0)
        pixel: fn() {
            let r = self.rect_size
            let p = self.pos * r
            let aa = 1.2

            // Per-segment fill colors from the themed ramp: 3 luminance stops,
            // pattern 1,3,3,2,2,1 (hi lightest, lo darkest), left-to-right.
            let k1 = vec3(self.seg_hi.x, self.seg_hi.y, self.seg_hi.z)
            let k2 = vec3(self.seg_lo.x, self.seg_lo.y, self.seg_lo.z)
            let k3 = vec3(self.seg_lo.x, self.seg_lo.y, self.seg_lo.z)
            let k4 = vec3(self.seg_mid.x, self.seg_mid.y, self.seg_mid.z)
            let k5 = vec3(self.seg_mid.x, self.seg_mid.y, self.seg_mid.z)
            let k6 = vec3(self.seg_hi.x, self.seg_hi.y, self.seg_hi.z)

            // ---- animated recolor: hover shimmer (mode 0) OR splash letter
            // pulse (modes >=1) ----------------------------------------------
            // Mode 0 is the top-bar wordmark's hover shimmer (unchanged). Modes
            // 1..6 are the always-on splash "WAML" letter pulse (start_screen.rs).
            // Every mode's math is computed, then selected branchlessly by the
            // per-mode weights m0..m6 (exactly one is 1) -- the DSL compiles no
            // `if`. Overlapping letter->segment map for the pulse variants:
            //   W = 1,2,3,4   A = 2,3   M = 2,3,4,5   L = 5,6
            let acc = vec3(self.accent.x, self.accent.y, self.accent.z)

            // -- mode 0: hover traveling-wave breathe (drives g1..g6) --
            let W = 4.0
            let PHI = 1.05
            let SPEED = 0.40
            let WIDTH = 0.02
            let tw = self.time * SPEED
            let phase = tw - floor(tw)
            let g1 = self.hover * clamp(0.45 * (0.5 + 0.5 * sin(self.time * W - 0.0 * PHI)) + exp(0.0 - (0.15 - phase) * (0.15 - phase) / WIDTH), 0.0, 1.0)
            let g2 = self.hover * clamp(0.45 * (0.5 + 0.5 * sin(self.time * W - 1.0 * PHI)) + exp(0.0 - (0.31 - phase) * (0.31 - phase) / WIDTH), 0.0, 1.0)
            let g3 = self.hover * clamp(0.45 * (0.5 + 0.5 * sin(self.time * W - 2.0 * PHI)) + exp(0.0 - (0.47 - phase) * (0.47 - phase) / WIDTH), 0.0, 1.0)
            let g4 = self.hover * clamp(0.45 * (0.5 + 0.5 * sin(self.time * W - 3.0 * PHI)) + exp(0.0 - (0.62 - phase) * (0.62 - phase) / WIDTH), 0.0, 1.0)
            let g5 = self.hover * clamp(0.45 * (0.5 + 0.5 * sin(self.time * W - 4.0 * PHI)) + exp(0.0 - (0.75 - phase) * (0.75 - phase) / WIDTH), 0.0, 1.0)
            let g6 = self.hover * clamp(0.45 * (0.5 + 0.5 * sin(self.time * W - 5.0 * PHI)) + exp(0.0 - (0.88 - phase) * (0.88 - phase) / WIDTH), 0.0, 1.0)

            // -- modes 1-3: shared W->A->M->L letter thump levels --
            // Each letter is a gaussian pulse centred in the loop; a segment's
            // level is the max over the letters that contain it. Secondary bleed
            // (slow breathe) + per-bar flicker fill the idle bars at low
            // amplitude (kept under the thump) so the mark never goes dead.
            let seqT = 3.2
            let su = self.time / seqT - floor(self.time / seqT)
            let wdt = 0.014
            let dW = su - 0.10
            let dA = su - 0.32
            let dM = su - 0.54
            let dL = su - 0.76
            let lvW = exp(0.0 - dW * dW / wdt)
            let lvA = exp(0.0 - dA * dA / wdt)
            let lvM = exp(0.0 - dM * dM / wdt)
            let lvL = exp(0.0 - dL * dL / wdt)
            let th1 = lvW
            let th2 = max(max(lvW, lvA), lvM)
            let th3 = max(max(lvW, lvA), lvM)
            let th4 = max(lvW, lvM)
            let th5 = max(lvM, lvL)
            let th6 = lvL
            let amb = 0.05 * (0.5 + 0.5 * sin(self.time * 2.3))
            let fsd = floor(self.time * 14.0)
            let fh1 = sin((fsd + 1.0) * 12.9898) * 43758.5453
            let fh2 = sin((fsd + 2.0) * 12.9898) * 43758.5453
            let fh3 = sin((fsd + 3.0) * 12.9898) * 43758.5453
            let fh4 = sin((fsd + 4.0) * 12.9898) * 43758.5453
            let fh5 = sin((fsd + 5.0) * 12.9898) * 43758.5453
            let fh6 = sin((fsd + 6.0) * 12.9898) * 43758.5453
            let fl1 = 0.12 * (fh1 - floor(fh1))
            let fl2 = 0.12 * (fh2 - floor(fh2))
            let fl3 = 0.12 * (fh3 - floor(fh3))
            let fl4 = 0.12 * (fh4 - floor(fh4))
            let fl5 = 0.12 * (fh5 - floor(fh5))
            let fl6 = 0.12 * (fh6 - floor(fh6))
            let bl1 = th1 + (1.0 - th1) * (amb + fl1)
            let bl2 = th2 + (1.0 - th2) * (amb + fl2)
            let bl3 = th3 + (1.0 - th3) * (amb + fl3)
            let bl4 = th4 + (1.0 - th4) * (amb + fl4)
            let bl5 = th5 + (1.0 - th5) * (amb + fl5)
            let bl6 = th6 + (1.0 - th6) * (amb + fl6)

            // mode 2 palette: Close Encounters light-organ (per-letter hue,
            // blended per segment by each contributing letter's live level; the
            // acc*eps term settles idle bars to accent instead of black).
            let ceW = vec3(0.90, 0.14, 0.11)
            let ceA = vec3(0.96, 0.56, 0.12)
            let ceM = vec3(0.18, 0.82, 0.38)
            let ceL = vec3(0.47, 0.32, 0.96)
            let eps = 0.0001
            let ce1 = (ceW * lvW + acc * eps) / (lvW + eps)
            let ce2 = (ceW * lvW + ceA * lvA + ceM * lvM + acc * eps) / (lvW + lvA + lvM + eps)
            let ce3 = ce2
            let ce4 = (ceW * lvW + ceM * lvM + acc * eps) / (lvW + lvM + eps)
            let ce5 = (ceM * lvM + ceL * lvL + acc * eps) / (lvM + lvL + eps)
            let ce6 = (ceL * lvL + acc * eps) / (lvL + eps)

            // mode 3 palette: our bucket swatches (Interface/UseCase/Package/
            // Behavior = blue/amber/green/pink), same per-seg blend.
            let pkW = vec3(0.078, 0.588, 0.863)
            let pkA = vec3(0.902, 0.588, 0.078)
            let pkM = vec3(0.235, 0.745, 0.353)
            let pkL = vec3(0.922, 0.275, 0.471)
            let pk1 = (pkW * lvW + acc * eps) / (lvW + eps)
            let pk2 = (pkW * lvW + pkA * lvA + pkM * lvM + acc * eps) / (lvW + lvA + lvM + eps)
            let pk3 = pk2
            let pk4 = (pkW * lvW + pkM * lvM + acc * eps) / (lvW + lvM + eps)
            let pk5 = (pkM * lvM + pkL * lvL + acc * eps) / (lvM + lvL + eps)
            let pk6 = (pkL * lvL + acc * eps) / (lvL + eps)

            // ================= AGENT VARIANTS (modes 4-6) =================
            // Authored by subagents; `fract`->`x-floor(x)`, `pow`->products,
            // `smoothstep`->inlined cubic to stay on the fork's proven intrinsic
            // set. Locals namespaced q4/q5/q6. Each yields a4/a5/a6 lev+colour.
            // Bar x-centres reused throughout: 0.16 0.32 0.47 0.62 0.75 0.89.

            // ---- mode 4: MOLTEN ("Molten Wordmark"): viscous magenta->violet
            // ->teal ooze, hot-gold core on the letter thump ----
            let q4Mag = vec3(0.95, 0.10, 0.52)
            let q4Vio = vec3(0.36, 0.09, 0.82)
            let q4Hot = vec3(1.00, 0.93, 0.70)
            let q4bw = 0.010
            let q4c = self.time * 0.18
            let q4cyc = q4c - floor(q4c)
            let q4pW = q4cyc - 0.125 + 0.5
            let q4pA = q4cyc - 0.375 + 0.5
            let q4pM = q4cyc - 0.625 + 0.5
            let q4pL = q4cyc - 0.875 + 0.5
            let q4dW = abs((q4pW - floor(q4pW)) - 0.5)
            let q4dA = abs((q4pA - floor(q4pA)) - 0.5)
            let q4dM = abs((q4pM - floor(q4pM)) - 0.5)
            let q4dL = abs((q4pL - floor(q4pL)) - 0.5)
            let q4eW = exp(0.0 - q4dW * q4dW / q4bw)
            let q4eA = exp(0.0 - q4dA * q4dA / q4bw)
            let q4eM = exp(0.0 - q4dM * q4dM / q4bw)
            let q4eL = exp(0.0 - q4dL * q4dL / q4bw)
            let q4L1 = q4eW
            let q4L2 = max(q4eW, max(q4eA, q4eM))
            let q4L3 = max(q4eW, max(q4eA, q4eM))
            let q4L4 = max(q4eW, q4eM)
            let q4L5 = max(q4eM, q4eL)
            let q4L6 = q4eL
            let q4b1 = 0.22 * (0.5 + 0.5 * sin(0.16 * 7.0 - self.time * 1.7)) * (0.5 + 0.5 * sin(0.16 * 3.3 + self.time * 0.6 + 1.7))
            let q4b2 = 0.22 * (0.5 + 0.5 * sin(0.32 * 7.0 - self.time * 1.7)) * (0.5 + 0.5 * sin(0.32 * 3.3 + self.time * 0.6 + 1.7))
            let q4b3 = 0.22 * (0.5 + 0.5 * sin(0.47 * 7.0 - self.time * 1.7)) * (0.5 + 0.5 * sin(0.47 * 3.3 + self.time * 0.6 + 1.7))
            let q4b4 = 0.22 * (0.5 + 0.5 * sin(0.62 * 7.0 - self.time * 1.7)) * (0.5 + 0.5 * sin(0.62 * 3.3 + self.time * 0.6 + 1.7))
            let q4b5 = 0.22 * (0.5 + 0.5 * sin(0.75 * 7.0 - self.time * 1.7)) * (0.5 + 0.5 * sin(0.75 * 3.3 + self.time * 0.6 + 1.7))
            let q4b6 = 0.22 * (0.5 + 0.5 * sin(0.89 * 7.0 - self.time * 1.7)) * (0.5 + 0.5 * sin(0.89 * 3.3 + self.time * 0.6 + 1.7))
            let q4tq = floor(self.time * 5.0)
            let q4h1 = sin((q4tq + 0.16 * 40.0) * 12.9898) * 43758.5453
            let q4h2 = sin((q4tq + 0.32 * 40.0) * 12.9898) * 43758.5453
            let q4h3 = sin((q4tq + 0.47 * 40.0) * 12.9898) * 43758.5453
            let q4h4 = sin((q4tq + 0.62 * 40.0) * 12.9898) * 43758.5453
            let q4h5 = sin((q4tq + 0.75 * 40.0) * 12.9898) * 43758.5453
            let q4h6 = sin((q4tq + 0.89 * 40.0) * 12.9898) * 43758.5453
            let q4k1 = 0.05 * (q4h1 - floor(q4h1))
            let q4k2 = 0.05 * (q4h2 - floor(q4h2))
            let q4k3 = 0.05 * (q4h3 - floor(q4h3))
            let q4k4 = 0.05 * (q4h4 - floor(q4h4))
            let q4k5 = 0.05 * (q4h5 - floor(q4h5))
            let q4k6 = 0.05 * (q4h6 - floor(q4h6))
            let a4lev1 = clamp(max(q4L1, q4b1) + q4k1, 0.0, 1.0)
            let a4lev2 = clamp(max(q4L2, q4b2) + q4k2, 0.0, 1.0)
            let a4lev3 = clamp(max(q4L3, q4b3) + q4k3, 0.0, 1.0)
            let a4lev4 = clamp(max(q4L4, q4b4) + q4k4, 0.0, 1.0)
            let a4lev5 = clamp(max(q4L5, q4b5) + q4k5, 0.0, 1.0)
            let a4lev6 = clamp(max(q4L6, q4b6) + q4k6, 0.0, 1.0)
            let q4a1 = 0.16 * 6.5 - self.time * 0.9
            let q4a2 = 0.32 * 6.5 - self.time * 0.9
            let q4a3 = 0.47 * 6.5 - self.time * 0.9
            let q4a4 = 0.62 * 6.5 - self.time * 0.9
            let q4a5 = 0.75 * 6.5 - self.time * 0.9
            let q4a6 = 0.89 * 6.5 - self.time * 0.9
            let q4B1 = mix(mix(q4Mag, q4Vio, 0.5 + 0.5 * sin(q4a1)), acc, (0.5 + 0.5 * sin(q4a1 * 0.6 + 2.1)) * 0.55)
            let q4B2 = mix(mix(q4Mag, q4Vio, 0.5 + 0.5 * sin(q4a2)), acc, (0.5 + 0.5 * sin(q4a2 * 0.6 + 2.1)) * 0.55)
            let q4B3 = mix(mix(q4Mag, q4Vio, 0.5 + 0.5 * sin(q4a3)), acc, (0.5 + 0.5 * sin(q4a3 * 0.6 + 2.1)) * 0.55)
            let q4B4 = mix(mix(q4Mag, q4Vio, 0.5 + 0.5 * sin(q4a4)), acc, (0.5 + 0.5 * sin(q4a4 * 0.6 + 2.1)) * 0.55)
            let q4B5 = mix(mix(q4Mag, q4Vio, 0.5 + 0.5 * sin(q4a5)), acc, (0.5 + 0.5 * sin(q4a5 * 0.6 + 2.1)) * 0.55)
            let q4B6 = mix(mix(q4Mag, q4Vio, 0.5 + 0.5 * sin(q4a6)), acc, (0.5 + 0.5 * sin(q4a6 * 0.6 + 2.1)) * 0.55)
            let a4c1 = mix(q4B1, q4Hot, q4L1 * q4L1)
            let a4c2 = mix(q4B2, q4Hot, q4L2 * q4L2)
            let a4c3 = mix(q4B3, q4Hot, q4L3 * q4L3)
            let a4c4 = mix(q4B4, q4Hot, q4L4 * q4L4)
            let a4c5 = mix(q4B5, q4Hot, q4L5 * q4L5)
            let a4c6 = mix(q4B6, q4Hot, q4L6 * q4L6)

            // ---- mode 5: NEON ("Neon Sign Ignition"): magenta<->cyan chroma
            // sweep, white-hot strike, buzz + VHS flicker ----
            let q5mag = vec3(1.00, 0.10, 0.70)
            let q5cyn = vec3(0.12, 0.95, 1.00)
            let q5cor = vec3(1.00, 0.88, 1.00)
            let q5bw = 0.010
            let q5p = self.time / 3.2
            let q5ph = q5p - floor(q5p)
            let q5eW0 = q5ph - 0.125 + 0.5
            let q5eA0 = q5ph - 0.375 + 0.5
            let q5eM0 = q5ph - 0.625 + 0.5
            let q5eL0 = q5ph - 0.875 + 0.5
            let q5dW = (q5eW0 - floor(q5eW0)) - 0.5
            let q5dA = (q5eA0 - floor(q5eA0)) - 0.5
            let q5dM = (q5eM0 - floor(q5eM0)) - 0.5
            let q5dL = (q5eL0 - floor(q5eL0)) - 0.5
            let q5eW = exp(0.0 - q5dW * q5dW / q5bw)
            let q5eA = exp(0.0 - q5dA * q5dA / q5bw)
            let q5eM = exp(0.0 - q5dM * q5dM / q5bw)
            let q5eL = exp(0.0 - q5dL * q5dL / q5bw)
            let q5t1 = q5eW
            let q5t2 = max(max(q5eW, q5eA), q5eM)
            let q5t3 = max(max(q5eW, q5eA), q5eM)
            let q5t4 = max(q5eW, q5eM)
            let q5t5 = max(q5eM, q5eL)
            let q5t6 = q5eL
            let q5z1 = 0.55 + 0.45 * sin(self.time * 8.0 + 0.16 * 6.2832)
            let q5z2 = 0.55 + 0.45 * sin(self.time * 8.0 + 0.32 * 6.2832)
            let q5z3 = 0.55 + 0.45 * sin(self.time * 8.0 + 0.47 * 6.2832)
            let q5z4 = 0.55 + 0.45 * sin(self.time * 8.0 + 0.62 * 6.2832)
            let q5z5 = 0.55 + 0.45 * sin(self.time * 8.0 + 0.75 * 6.2832)
            let q5z6 = 0.55 + 0.45 * sin(self.time * 8.0 + 0.89 * 6.2832)
            let q5f = floor(self.time * 12.0)
            let q5h1 = sin((q5f + 0.16 * 57.0) * 12.9898) * 43758.5453
            let q5h2 = sin((q5f + 0.32 * 57.0) * 12.9898) * 43758.5453
            let q5h3 = sin((q5f + 0.47 * 57.0) * 12.9898) * 43758.5453
            let q5h4 = sin((q5f + 0.62 * 57.0) * 12.9898) * 43758.5453
            let q5h5 = sin((q5f + 0.75 * 57.0) * 12.9898) * 43758.5453
            let q5h6 = sin((q5f + 0.89 * 57.0) * 12.9898) * 43758.5453
            let q5k1 = q5h1 - floor(q5h1)
            let q5k2 = q5h2 - floor(q5h2)
            let q5k3 = q5h3 - floor(q5h3)
            let q5k4 = q5h4 - floor(q5h4)
            let q5k5 = q5h5 - floor(q5h5)
            let q5k6 = q5h6 - floor(q5h6)
            let a5lev1 = clamp(q5t1 + 0.16 * (0.6 * q5z1 + 0.4 * q5k1), 0.0, 1.0)
            let a5lev2 = clamp(q5t2 + 0.16 * (0.6 * q5z2 + 0.4 * q5k2), 0.0, 1.0)
            let a5lev3 = clamp(q5t3 + 0.16 * (0.6 * q5z3 + 0.4 * q5k3), 0.0, 1.0)
            let a5lev4 = clamp(q5t4 + 0.16 * (0.6 * q5z4 + 0.4 * q5k4), 0.0, 1.0)
            let a5lev5 = clamp(q5t5 + 0.16 * (0.6 * q5z5 + 0.4 * q5k5), 0.0, 1.0)
            let a5lev6 = clamp(q5t6 + 0.16 * (0.6 * q5z6 + 0.4 * q5k6), 0.0, 1.0)
            let q5g1 = 0.5 + 0.5 * sin(self.time * 0.8 + 0.16 * 3.5)
            let q5g2 = 0.5 + 0.5 * sin(self.time * 0.8 + 0.32 * 3.5)
            let q5g3 = 0.5 + 0.5 * sin(self.time * 0.8 + 0.47 * 3.5)
            let q5g4 = 0.5 + 0.5 * sin(self.time * 0.8 + 0.62 * 3.5)
            let q5g5 = 0.5 + 0.5 * sin(self.time * 0.8 + 0.75 * 3.5)
            let q5g6 = 0.5 + 0.5 * sin(self.time * 0.8 + 0.89 * 3.5)
            let a5c1 = mix(mix(q5mag, q5cyn, q5g1), q5cor, q5t1 * q5t1 * q5t1 * 0.6)
            let a5c2 = mix(mix(q5mag, q5cyn, q5g2), q5cor, q5t2 * q5t2 * q5t2 * 0.6)
            let a5c3 = mix(mix(q5mag, q5cyn, q5g3), q5cor, q5t3 * q5t3 * q5t3 * 0.6)
            let a5c4 = mix(mix(q5mag, q5cyn, q5g4), q5cor, q5t4 * q5t4 * q5t4 * 0.6)
            let a5c5 = mix(mix(q5mag, q5cyn, q5g5), q5cor, q5t5 * q5t5 * q5t5 * 0.6)
            let a5c6 = mix(mix(q5mag, q5cyn, q5g6), q5cor, q5t6 * q5t6 * q5t6 * 0.6)

            // ---- mode 6: ELECTRIC ("Capacitor Crack"): cyan corona -> cold
            // white crack, current arc bar-to-bar, rare violet spark ----
            let q6wht = vec3(0.86, 0.96, 1.00)
            let q6vio = vec3(0.60, 0.36, 0.98)
            let q6pp = self.time * 0.32
            let q6ph = q6pp - floor(q6pp)
            let q6eW0 = q6ph - 0.125 + 0.5
            let q6eA0 = q6ph - 0.375 + 0.5
            let q6eM0 = q6ph - 0.625 + 0.5
            let q6eL0 = q6ph - 0.875 + 0.5
            let q6dW = (q6eW0 - floor(q6eW0)) - 0.5
            let q6dA = (q6eA0 - floor(q6eA0)) - 0.5
            let q6dM = (q6eM0 - floor(q6eM0)) - 0.5
            let q6dL = (q6eL0 - floor(q6eL0)) - 0.5
            let q6envW = 0.82 * exp(0.0 - q6dW * q6dW / 0.011) + 0.55 * exp(0.0 - q6dW * q6dW / 0.0019)
            let q6envA = 0.82 * exp(0.0 - q6dA * q6dA / 0.011) + 0.55 * exp(0.0 - q6dA * q6dA / 0.0019)
            let q6envM = 0.82 * exp(0.0 - q6dM * q6dM / 0.011) + 0.55 * exp(0.0 - q6dM * q6dM / 0.0019)
            let q6envL = 0.82 * exp(0.0 - q6dL * q6dL / 0.011) + 0.55 * exp(0.0 - q6dL * q6dL / 0.0019)
            let q6L1 = q6envW
            let q6L2 = max(max(q6envW, q6envA), q6envM)
            let q6L3 = max(max(q6envW, q6envA), q6envM)
            let q6L4 = max(q6envW, q6envM)
            let q6L5 = max(q6envM, q6envL)
            let q6L6 = q6envL
            let q6ax0 = self.time * 0.9
            let q6ax = q6ax0 - floor(q6ax0)
            let q6arc1 = 0.34 * exp(0.0 - (0.16 - q6ax) * (0.16 - q6ax) / 0.0045)
            let q6arc2 = 0.34 * exp(0.0 - (0.32 - q6ax) * (0.32 - q6ax) / 0.0045)
            let q6arc3 = 0.34 * exp(0.0 - (0.47 - q6ax) * (0.47 - q6ax) / 0.0045)
            let q6arc4 = 0.34 * exp(0.0 - (0.62 - q6ax) * (0.62 - q6ax) / 0.0045)
            let q6arc5 = 0.34 * exp(0.0 - (0.75 - q6ax) * (0.75 - q6ax) / 0.0045)
            let q6arc6 = 0.34 * exp(0.0 - (0.89 - q6ax) * (0.89 - q6ax) / 0.0045)
            let q6ft = floor(self.time * 24.0)
            let q6fh1 = sin((q6ft + 1.0 * 57.0) * 12.9898) * 43758.5453
            let q6fh2 = sin((q6ft + 2.0 * 57.0) * 12.9898) * 43758.5453
            let q6fh3 = sin((q6ft + 3.0 * 57.0) * 12.9898) * 43758.5453
            let q6fh4 = sin((q6ft + 4.0 * 57.0) * 12.9898) * 43758.5453
            let q6fh5 = sin((q6ft + 5.0 * 57.0) * 12.9898) * 43758.5453
            let q6fh6 = sin((q6ft + 6.0 * 57.0) * 12.9898) * 43758.5453
            let q6flk1 = q6fh1 - floor(q6fh1)
            let q6flk2 = q6fh2 - floor(q6fh2)
            let q6flk3 = q6fh3 - floor(q6fh3)
            let q6flk4 = q6fh4 - floor(q6fh4)
            let q6flk5 = q6fh5 - floor(q6fh5)
            let q6flk6 = q6fh6 - floor(q6fh6)
            let a6lev1 = clamp(q6L1 + q6arc1 + 0.14 * q6flk1, 0.0, 1.0)
            let a6lev2 = clamp(q6L2 + q6arc2 + 0.14 * q6flk2, 0.0, 1.0)
            let a6lev3 = clamp(q6L3 + q6arc3 + 0.14 * q6flk3, 0.0, 1.0)
            let a6lev4 = clamp(q6L4 + q6arc4 + 0.14 * q6flk4, 0.0, 1.0)
            let a6lev5 = clamp(q6L5 + q6arc5 + 0.14 * q6flk5, 0.0, 1.0)
            let a6lev6 = clamp(q6L6 + q6arc6 + 0.14 * q6flk6, 0.0, 1.0)
            let q6st = floor(self.time * 33.0)
            let q6sh1 = sin((q6st + 1.0 * 17.0) * 78.233) * 43758.5453
            let q6sh2 = sin((q6st + 2.0 * 17.0) * 78.233) * 43758.5453
            let q6sh3 = sin((q6st + 3.0 * 17.0) * 78.233) * 43758.5453
            let q6sh4 = sin((q6st + 4.0 * 17.0) * 78.233) * 43758.5453
            let q6sh5 = sin((q6st + 5.0 * 17.0) * 78.233) * 43758.5453
            let q6sh6 = sin((q6st + 6.0 * 17.0) * 78.233) * 43758.5453
            let q6spk1 = q6sh1 - floor(q6sh1)
            let q6spk2 = q6sh2 - floor(q6sh2)
            let q6spk3 = q6sh3 - floor(q6sh3)
            let q6spk4 = q6sh4 - floor(q6sh4)
            let q6spk5 = q6sh5 - floor(q6sh5)
            let q6spk6 = q6sh6 - floor(q6sh6)
            let q6r1 = clamp((a6lev1 - 0.5) / 0.46, 0.0, 1.0)
            let q6r2 = clamp((a6lev2 - 0.5) / 0.46, 0.0, 1.0)
            let q6r3 = clamp((a6lev3 - 0.5) / 0.46, 0.0, 1.0)
            let q6r4 = clamp((a6lev4 - 0.5) / 0.46, 0.0, 1.0)
            let q6r5 = clamp((a6lev5 - 0.5) / 0.46, 0.0, 1.0)
            let q6r6 = clamp((a6lev6 - 0.5) / 0.46, 0.0, 1.0)
            let q6sm1 = q6r1 * q6r1 * (3.0 - 2.0 * q6r1)
            let q6sm2 = q6r2 * q6r2 * (3.0 - 2.0 * q6r2)
            let q6sm3 = q6r3 * q6r3 * (3.0 - 2.0 * q6r3)
            let q6sm4 = q6r4 * q6r4 * (3.0 - 2.0 * q6r4)
            let q6sm5 = q6r5 * q6r5 * (3.0 - 2.0 * q6r5)
            let q6sm6 = q6r6 * q6r6 * (3.0 - 2.0 * q6r6)
            let q6n1 = clamp((q6spk1 - 0.72) / 0.28, 0.0, 1.0)
            let q6n2 = clamp((q6spk2 - 0.72) / 0.28, 0.0, 1.0)
            let q6n3 = clamp((q6spk3 - 0.72) / 0.28, 0.0, 1.0)
            let q6n4 = clamp((q6spk4 - 0.72) / 0.28, 0.0, 1.0)
            let q6n5 = clamp((q6spk5 - 0.72) / 0.28, 0.0, 1.0)
            let q6n6 = clamp((q6spk6 - 0.72) / 0.28, 0.0, 1.0)
            let q6sp1 = q6n1 * q6n1 * (3.0 - 2.0 * q6n1)
            let q6sp2 = q6n2 * q6n2 * (3.0 - 2.0 * q6n2)
            let q6sp3 = q6n3 * q6n3 * (3.0 - 2.0 * q6n3)
            let q6sp4 = q6n4 * q6n4 * (3.0 - 2.0 * q6n4)
            let q6sp5 = q6n5 * q6n5 * (3.0 - 2.0 * q6n5)
            let q6sp6 = q6n6 * q6n6 * (3.0 - 2.0 * q6n6)
            let a6c1 = mix(mix(acc, q6wht, q6sm1), q6vio, q6sp1 * 0.55 * a6lev1)
            let a6c2 = mix(mix(acc, q6wht, q6sm2), q6vio, q6sp2 * 0.55 * a6lev2)
            let a6c3 = mix(mix(acc, q6wht, q6sm3), q6vio, q6sp3 * 0.55 * a6lev3)
            let a6c4 = mix(mix(acc, q6wht, q6sm4), q6vio, q6sp4 * 0.55 * a6lev4)
            let a6c5 = mix(mix(acc, q6wht, q6sm5), q6vio, q6sp5 * 0.55 * a6lev5)
            let a6c6 = mix(mix(acc, q6wht, q6sm6), q6vio, q6sp6 * 0.55 * a6lev6)

            // -- per-mode selector weights (exactly one is 1; no `if` in DSL) --
            let s05 = sign(self.mode - 0.5)
            let s15 = sign(self.mode - 1.5)
            let s25 = sign(self.mode - 2.5)
            let s35 = sign(self.mode - 3.5)
            let s45 = sign(self.mode - 4.5)
            let s55 = sign(self.mode - 5.5)
            let s65 = sign(self.mode - 6.5)
            let m0 = 0.5 - 0.5 * s05
            let m1 = (0.5 + 0.5 * s05) * (0.5 - 0.5 * s15)
            let m2 = (0.5 + 0.5 * s15) * (0.5 - 0.5 * s25)
            let m3 = (0.5 + 0.5 * s25) * (0.5 - 0.5 * s35)
            let m4 = (0.5 + 0.5 * s35) * (0.5 - 0.5 * s45)
            let m5 = (0.5 + 0.5 * s45) * (0.5 - 0.5 * s55)
            let m6 = (0.5 + 0.5 * s55) * (0.5 - 0.5 * s65)
            let seq = m1 + m2 + m3

            // -- resolve per-segment level + target colour, then recolor --
            let lev1 = m0 * g1 + seq * bl1 + m4 * a4lev1 + m5 * a5lev1 + m6 * a6lev1
            let lev2 = m0 * g2 + seq * bl2 + m4 * a4lev2 + m5 * a5lev2 + m6 * a6lev2
            let lev3 = m0 * g3 + seq * bl3 + m4 * a4lev3 + m5 * a5lev3 + m6 * a6lev3
            let lev4 = m0 * g4 + seq * bl4 + m4 * a4lev4 + m5 * a5lev4 + m6 * a6lev4
            let lev5 = m0 * g5 + seq * bl5 + m4 * a4lev5 + m5 * a5lev5 + m6 * a6lev5
            let lev6 = m0 * g6 + seq * bl6 + m4 * a4lev6 + m5 * a5lev6 + m6 * a6lev6
            let tc1 = (m0 + m1) * acc + m2 * ce1 + m3 * pk1 + m4 * a4c1 + m5 * a5c1 + m6 * a6c1
            let tc2 = (m0 + m1) * acc + m2 * ce2 + m3 * pk2 + m4 * a4c2 + m5 * a5c2 + m6 * a6c2
            let tc3 = (m0 + m1) * acc + m2 * ce3 + m3 * pk3 + m4 * a4c3 + m5 * a5c3 + m6 * a6c3
            let tc4 = (m0 + m1) * acc + m2 * ce4 + m3 * pk4 + m4 * a4c4 + m5 * a5c4 + m6 * a6c4
            let tc5 = (m0 + m1) * acc + m2 * ce5 + m3 * pk5 + m4 * a4c5 + m5 * a5c5 + m6 * a6c5
            let tc6 = (m0 + m1) * acc + m2 * ce6 + m3 * pk6 + m4 * a4c6 + m5 * a5c6 + m6 * a6c6
            let kg1 = mix(k1, tc1, clamp(lev1, 0.0, 1.0))
            let kg2 = mix(k2, tc2, clamp(lev2, 0.0, 1.0))
            let kg3 = mix(k3, tc3, clamp(lev3, 0.0, 1.0))
            let kg4 = mix(k4, tc4, clamp(lev4, 0.0, 1.0))
            let kg5 = mix(k5, tc5, clamp(lev5, 0.0, 1.0))
            let kg6 = mix(k6, tc6, clamp(lev6, 0.0, 1.0))

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

            // Per-bar AA coverage (0.5 exactly on each bar's own edge).
            let cc1 = clamp(0.5 - dq1 / aa, 0.0, 1.0)
            let cc2 = clamp(0.5 - dq2 / aa, 0.0, 1.0)
            let cc3 = clamp(0.5 - dq3 / aa, 0.0, 1.0)
            let cc4 = clamp(0.5 - dq4 / aa, 0.0, 1.0)
            let cc5 = clamp(0.5 - dq5 / aa, 0.0, 1.0)
            let cc6 = clamp(0.5 - dq6 / aa, 0.0, 1.0)

            // COLOR: fold-order "over" composite (2,4,6 then 1,3,5 on top) so
            // the two shades blend smoothly across each internal seam. Its own
            // alpha dips at seams, so we keep only its un-premultiplied color.
            let acc2 = vec4(kg2, 1.0) * cc2
            let acc4 = vec4(kg4, 1.0) * cc4 + acc2 * (1.0 - cc4)
            let acc6 = vec4(kg6, 1.0) * cc6 + acc4 * (1.0 - cc6)
            let acc1 = vec4(kg1, 1.0) * cc1 + acc6 * (1.0 - cc1)
            let acc3 = vec4(kg3, 1.0) * cc3 + acc1 * (1.0 - cc3)
            let acc5 = vec4(kg5, 1.0) * cc5 + acc3 * (1.0 - cc5)
            let straight = acc5.rgb / max(acc5.a, 0.0001)

            // ALPHA: silhouette coverage = sum of the TWO largest per-bar
            // coverages. On an internal seam the two abutting bars each read 0.5
            // and sum to 1 (no background hairline); on a true outer edge only
            // one bar contributes, giving normal AA -- and since nothing is
            // dilated, sharp tips stay sharp (no miter spikes). Track top two:
            let t1 = max(cc1, cc2)
            let t2 = min(cc1, cc2)
            let u1 = max(t1, cc3)
            let u2 = max(t2, min(t1, cc3))
            let v1 = max(u1, cc4)
            let v2 = max(u2, min(u1, cc4))
            let w1 = max(v1, cc5)
            let w2 = max(v2, min(v1, cc5))
            let x1 = max(w1, cc6)
            let x2 = max(w2, min(w1, cc6))
            let cover = clamp(x1 + x2, 0.0, 1.0)

            // Blended color, premultiplied by the hole-free silhouette coverage.
            // `fade` scales that coverage so the splash click-crossfade can draw
            // the incoming variant partially-transparent over the held outgoing.
            let fcover = cover * self.fade
            return vec4(straight * fcover, fcover)
        }
    }

    mod.widgets.LogoMarkBase = #(LogoMark::register_widget(vm))

    // The interactive top-bar wordmark widget. `draw_bg` is the SDF shader
    // above; the Rust `LogoMark` drives its `hover`/`time` uniforms from the
    // hover animation loop and emits `LogoAction::Clicked` on a primary press.
    mod.widgets.LogoMark = set_type_default() do mod.widgets.LogoMarkBase{
        width: Fill
        height: Fill
        draw_bg: mod.draw.LogoMark{}
    }
}

// Hover ease-in/out duration (seconds): `hover` ramps 0->1 on enter, 1->0 on
// leave over this window (screenshot-tuned along with the shimmer constants).
const HOVER_SECS: f64 = 0.15;

// Splash click-crossfade duration (seconds): a logo click cross-dissolves from
// the current colour variant to the next over this window.
const FADE_SECS: f64 = 0.4;

/// `LogoMark` -> `App` action (same convention as `GraphCanvasAction`). Carries
/// the wordmark's screen-space centre so `App` can open the radial there.
///
/// `#[allow(dead_code)]`: the `logo_harness` bin path-includes `logo.rs` without
/// the `App` wiring, so the payload/readers look unused in that unit.
#[allow(dead_code)]
#[derive(Clone, Debug, Default)]
pub enum LogoAction {
    #[default]
    None,
    Clicked(DVec2),
}

/// The interactive top-bar wordmark. Unlike `WamlButton`/`Radial` (event-passive
/// components driven by their parent), this is a self-routing `Widget`: it
/// hit-tests its own drawn area and runs a `NextFrame` hover-shimmer loop. Note
/// it only receives hover/click once `App` answers `WindowDragQueryResponse::
/// Client` over its `drawn_rect` (the caption-bar drag region swallows events
/// otherwise -- see `app.rs`).
#[derive(Script, ScriptHook, Widget)]
pub struct LogoMark {
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
    draw_bg: DrawQuad,

    // When true (the splash logo), the mark self-animates unconditionally --
    // it runs its `mode` pulse on a free-running clock and skips the cursor +
    // click behavior. Defaults false so the top-bar wordmark keeps its
    // hover-gated shimmer and `Clicked` action.
    #[live]
    auto: bool,

    // Splash colour-pulse variant (1..6). `#[live]` so the start-screen sets the
    // initial variant; clicking the splash advances it (1->6, wrapping) and this
    // field drives the shader `mode` uniform from `draw_walk`. Non-splash
    // instances leave it 0 (the hover-shimmer mode).
    #[live]
    mode: f32,

    // Pointer is over the mark.
    #[rust]
    hovered: bool,
    // Eased hover, 0..1, fed to the shader `hover` uniform.
    #[rust]
    hover: f32,
    // `time` uniform value (seconds since the last hover-in) -- the shimmer
    // wave/breathe clock.
    #[rust]
    time: f32,
    // Wall-clock origin for `time`, reset on each hover-in.
    #[rust]
    anim_start: f64,
    // Last next-frame timestamp, for frame-rate-independent easing.
    #[rust]
    last_time: f64,
    // Last drawn rect (absolute) -- exposed for the drag-query override.
    #[rust]
    rect: Rect,
    // Click-crossfade state (splash/`auto` only): `prev_mode` is the outgoing
    // variant held at full opacity while `fade_t` ramps 0->1 fading the new
    // `mode` in over it; `fading` gates the two-pass draw in `draw_walk`.
    #[rust]
    prev_mode: f32,
    #[rust]
    fade_t: f32,
    #[rust]
    fading: bool,
    #[rust]
    next_frame: NextFrame,
}

impl Widget for LogoMark {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, _scope: &mut Scope) {
        // Hover-shimmer animation: ease `hover` toward the target and advance the
        // shimmer clock while active. Idle (hover==0, not hovered) stops
        // scheduling frames -> zero cost.
        if let Some(ne) = self.next_frame.is_event(event) {
            if self.auto {
                // Always-on splash pulse: advance the free-running clock and
                // keep the frame loop alive. Hover easing is unused here.
                self.time = ne.time as f32;
                // Advance an in-flight click crossfade toward the new variant.
                if self.fading {
                    let dt = (ne.time - self.last_time).max(0.0);
                    self.fade_t = (self.fade_t + (dt / FADE_SECS) as f32).min(1.0);
                    if self.fade_t >= 1.0 {
                        self.fading = false;
                    }
                }
                self.last_time = ne.time;
                self.next_frame = cx.new_next_frame();
                self.draw_bg.redraw(cx);
            } else {
                let target = if self.hovered { 1.0 } else { 0.0 };
                let dt = (ne.time - self.last_time).max(0.0);
                self.last_time = ne.time;
                let step = (dt / HOVER_SECS) as f32;
                if self.hover < target {
                    self.hover = (self.hover + step).min(target);
                } else if self.hover > target {
                    self.hover = (self.hover - step).max(target);
                }
                self.time = (ne.time - self.anim_start) as f32;
                if self.hovered || self.hover > 0.0 {
                    self.next_frame = cx.new_next_frame();
                }
                self.draw_bg.redraw(cx);
            }
        }

        // Splash (auto) instances ARE clickable, to cycle colour variants: a
        // Hand cursor advertises it and a primary press advances `mode` (1..6,
        // wrapping) then kicks off a crossfade. Unlike the top-bar mark this
        // emits no `LogoAction` -- the click is consumed here.
        if self.auto {
            match event.hits(cx, self.draw_bg.area()) {
                Hit::FingerHoverIn(_) | Hit::FingerHoverOver(_) => {
                    cx.set_cursor(MouseCursor::Hand);
                }
                Hit::FingerDown(fe) if fe.is_primary_hit() => {
                    self.prev_mode = self.mode;
                    self.mode = self.mode % 6.0 + 1.0;
                    self.fade_t = 0.0;
                    self.fading = true;
                    self.last_time = cx.seconds_since_app_start();
                    self.next_frame = cx.new_next_frame();
                    self.draw_bg.redraw(cx);
                }
                _ => {}
            }
        }

        // Non-splash (top-bar) instances hit-test for the hover shimmer + the
        // `LogoAction::Clicked` that opens the radial.
        if !self.auto {
            let uid = self.widget_uid();
            match event.hits_with_capture_overload(cx, self.draw_bg.area(), true) {
                Hit::FingerHoverIn(_) | Hit::FingerHoverOver(_) => {
                    cx.set_cursor(MouseCursor::Hand);
                    if !self.hovered {
                        self.hovered = true;
                        let now = cx.seconds_since_app_start();
                        self.anim_start = now;
                        self.last_time = now;
                        self.next_frame = cx.new_next_frame();
                    }
                }
                Hit::FingerHoverOut(_) => {
                    if self.hovered {
                        self.hovered = false;
                        self.last_time = cx.seconds_since_app_start();
                        self.next_frame = cx.new_next_frame();
                    }
                }
                Hit::FingerDown(fe) if fe.is_primary_hit() => {
                    let center = dvec2(
                        self.rect.pos.x + self.rect.size.x * 0.5,
                        self.rect.pos.y + self.rect.size.y * 0.5,
                    );
                    cx.widget_action(uid, LogoAction::Clicked(center));
                }
                _ => {}
            }
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, _scope: &mut Scope, walk: Walk) -> DrawStep {
        let rect = cx.walk_turtle(walk);
        self.rect = rect;
        // Splash instances free-run: re-arm the frame loop every draw. NextFrame
        // tokens overwrite, so this never stacks; when the splash is hidden no
        // draw happens and the loop naturally pauses, resuming on the next draw.
        if self.auto {
            self.next_frame = cx.new_next_frame();
        }
        self.draw_bg.set_uniform(cx, live_id!(hover), &[self.hover]);
        self.draw_bg.set_uniform(cx, live_id!(time), &[self.time]);
        if self.fading {
            // Two-pass crossfade: outgoing variant at full coverage, then the
            // incoming one over it at `fade_t`. The two share the identical W
            // silhouette, so this reads as a colour cross-dissolve with a solid,
            // hole-free hull. Differing `mode`/`fade` uniforms make makepad break
            // the batch into two draw calls (see draw_list.rs uniform compare).
            self.draw_bg.set_uniform(cx, live_id!(mode), &[self.prev_mode]);
            self.draw_bg.set_uniform(cx, live_id!(fade), &[1.0]);
            self.draw_bg.draw_abs(cx, rect);
            self.draw_bg.set_uniform(cx, live_id!(mode), &[self.mode]);
            self.draw_bg.set_uniform(cx, live_id!(fade), &[self.fade_t]);
            self.draw_bg.draw_abs(cx, rect);
        } else {
            self.draw_bg.set_uniform(cx, live_id!(mode), &[self.mode]);
            self.draw_bg.set_uniform(cx, live_id!(fade), &[1.0]);
            self.draw_bg.draw_abs(cx, rect);
        }
        DrawStep::done()
    }
}

#[allow(dead_code)] // readers used by `App`; unused in the `logo_harness` bin.
impl LogoMark {
    /// The mark's last-drawn absolute rect. `App` uses this to answer the OS
    /// window drag-query as `Client`, so hover/click reach `handle_event`.
    pub fn drawn_rect(&self) -> Rect {
        self.rect
    }

    /// Reader for `App` (mirrors `GraphCanvas::canvas_action`): the wordmark
    /// centre if a `Clicked` action landed this frame, else `None`.
    pub fn logo_action(&self, actions: &Actions) -> Option<DVec2> {
        let item = actions.find_widget_action(self.widget_uid())?;
        match item.cast::<LogoAction>() {
            LogoAction::Clicked(center) => Some(center),
            LogoAction::None => None,
        }
    }
}
