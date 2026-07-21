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
        // any plain instance is unchanged); 1..7 = always-on splash letter
        // pulse variants (accent / Close Encounters / bucket-palette / the
        // agent-authored molten / neon / electric sets / and per-segment
        // desaturated-until-pulse). See `pixel`.
        mode: uniform(0.0)
        // Crossfade coverage scale (0..1), default 1 = solid. Only the splash
        // drives it below 1: on a logo click it draws the outgoing variant at
        // fade=1, then the incoming variant over it at fade=0->1, cross-
        // dissolving the two matched silhouettes. Every other instance leaves
        // it at rest.
        fade: uniform(1.0)
        // FPS-heat meter (top-bar wordmark only, independent of `mode`): the Rust
        // side samples framerate during a pointer interaction and drives these.
        // `fps_color` is the heat target (green->amber->red); `meter` (0..1) is the
        // eased strength every segment tints toward. Both default to rest (inert
        // grey / 0) so non-metered instances -- splash, harness, card -- are
        // pixel-identical to before.
        fps_color: uniform(vec3(0.5, 0.5, 0.5))
        meter: uniform(0.0)
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
            // 1..7 are the always-on splash "WAML" letter pulse (start_screen.rs).
            // Every mode's math is computed, then selected branchlessly by the
            // per-mode weights m0..m7 (exactly one is 1) -- the DSL compiles no
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

            // -- modes 1-3,7: shared W->A->M->L letter levels: hero then dance --
            // The loop is one long period (seqT): the hero W->A->M->L sweep is
            // packed into the FRONT ~40%, then after a short beat the letters
            // "dance" in a pseudo-random pattern through the back half, settling
            // before the seamless wrap. Every dance term is a harmonic of the loop
            // frequency (sin of integer*tau), so the whole cycle is EXACTLY
            // periodic -- the repeat has no jump. The "random" look is decorrelated
            // per-letter phase offsets, not real noise (real noise can't loop).
            // A segment's level is the max over the letters that contain it; the
            // colour blends (ce/pk/sg below) read the live letter levels, so each
            // mode dances in its own palette.
            let seqT = 6.4
            let su = self.time / seqT - floor(self.time / seqT)
            let tau = 6.2831853 * su
            // Hero gaussians. Centres 0.16..0.43 (spacing 0.09) so the FIRST
            // pulse is ~0 at su=0 (exp(-0.16^2/0.0035) ~ 0.0007) and the LAST is
            // ~0 by su=1 -- the whole hero term is flat-zero across the wrap, no
            // pop. wdt tuned so each pulse ~0.38s at this period.
            let wdt = 0.0035
            let dW = su - 0.16
            let dA = su - 0.25
            let dM = su - 0.34
            let dL = su - 0.43
            let hvW = exp(0.0 - dW * dW / wdt)
            let hvA = exp(0.0 - dA * dA / wdt)
            let hvM = exp(0.0 - dM * dM / wdt)
            let hvL = exp(0.0 - dL * dL / wdt)
            // Dance window: sin^2 bump over su 0.52..0.99 -- zero VALUE and SLOPE
            // at both ends (pi/0.47 = 6.6842), so it tapers smoothly to nothing
            // before the wrap and there is no kink where it meets the hero rest.
            // max(0,..) clips it to that interval.
            let dsd = max(0.0, sin(6.6842 * (su - 0.52)))
            let dwin = dsd * dsd
            let dnW = dwin * clamp(0.42 + 0.40 * sin(3.0 * tau + 0.0) + 0.22 * sin(8.0 * tau + 1.3), 0.0, 0.9)
            let dnA = dwin * clamp(0.42 + 0.40 * sin(3.0 * tau + 1.7) + 0.22 * sin(8.0 * tau + 3.1), 0.0, 0.9)
            let dnM = dwin * clamp(0.42 + 0.40 * sin(3.0 * tau + 3.4) + 0.22 * sin(8.0 * tau + 5.0), 0.0, 0.9)
            let dnL = dwin * clamp(0.42 + 0.40 * sin(3.0 * tau + 5.1) + 0.22 * sin(8.0 * tau + 0.7), 0.0, 0.9)
            // Live letter level = hero pulse OR dance, whichever is brighter.
            let lvW = max(hvW, dnW)
            let lvA = max(hvA, dnA)
            let lvM = max(hvM, dnM)
            let lvL = max(hvL, dnL)
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

            // mode 7 palette: a FIXED bucket hue per SEGMENT (not mode 3's
            // per-letter blend). Grey at rest is free -- the final mix(k,tc,lev)
            // fades each segment to the grey ramp as its thump level -> 0, so no
            // idle colour floor is added (that floor is exactly what keeps mode 3
            // idling accent instead of grey; mode 7 omits it). As a letter thumps,
            // each of its segments lights its OWN distinct hue, so the letter reads
            // as several colours at once. Spectrum left->right across the 7
            // AccentBucket swatches (inspector_panel.rs bucket_color): blue, cyan,
            // green, amber, pink, indigo -- 6 distinct so every letter (W=1,2,3,4
            // A=2,3  M=2,3,4,5  L=5,6) spans multiple hues.
            let sg1 = vec3(0.078, 0.588, 0.863)
            let sg2 = vec3(0.000, 0.706, 0.824)
            let sg3 = vec3(0.235, 0.745, 0.353)
            let sg4 = vec3(0.902, 0.588, 0.078)
            let sg5 = vec3(0.922, 0.275, 0.471)
            let sg6 = vec3(0.353, 0.431, 0.941)

            // ============ PALETTE VARIANTS (modes 4-6) ============
            // These reuse the shared hero/dance LEVEL above (th1..6), so every
            // mode pulses on the same W->A->M->L-then-dance rhythm and loops
            // seamlessly; they differ only in COLOUR. Hue oscillators are smooth
            // functions of absolute time (no wrap seam -- the level->0 there masks
            // them anyway). Bar x-centres: 0.16 0.32 0.47 0.62 0.75 0.89.

            // ---- mode 4: MOLTEN: viscous magenta->violet ooze drifting toward
            // accent, hot-gold core on the thump. ----
            let q4Mag = vec3(0.95, 0.10, 0.52)
            let q4Vio = vec3(0.36, 0.09, 0.82)
            let q4Hot = vec3(1.00, 0.93, 0.70)
            let a4lev1 = th1
            let a4lev2 = th2
            let a4lev3 = th3
            let a4lev4 = th4
            let a4lev5 = th5
            let a4lev6 = th6
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
            let a4c1 = mix(q4B1, q4Hot, th1 * th1)
            let a4c2 = mix(q4B2, q4Hot, th2 * th2)
            let a4c3 = mix(q4B3, q4Hot, th3 * th3)
            let a4c4 = mix(q4B4, q4Hot, th4 * th4)
            let a4c5 = mix(q4B5, q4Hot, th5 * th5)
            let a4c6 = mix(q4B6, q4Hot, th6 * th6)

            // ---- mode 5: NEON: magenta<->cyan chroma sweep, white-hot strike
            // core on the thump. ----
            let q5mag = vec3(1.00, 0.10, 0.70)
            let q5cyn = vec3(0.12, 0.95, 1.00)
            let q5cor = vec3(1.00, 0.88, 1.00)
            let a5lev1 = th1
            let a5lev2 = th2
            let a5lev3 = th3
            let a5lev4 = th4
            let a5lev5 = th5
            let a5lev6 = th6
            let q5g1 = 0.5 + 0.5 * sin(self.time * 0.8 + 0.16 * 3.5)
            let q5g2 = 0.5 + 0.5 * sin(self.time * 0.8 + 0.32 * 3.5)
            let q5g3 = 0.5 + 0.5 * sin(self.time * 0.8 + 0.47 * 3.5)
            let q5g4 = 0.5 + 0.5 * sin(self.time * 0.8 + 0.62 * 3.5)
            let q5g5 = 0.5 + 0.5 * sin(self.time * 0.8 + 0.75 * 3.5)
            let q5g6 = 0.5 + 0.5 * sin(self.time * 0.8 + 0.89 * 3.5)
            let a5c1 = mix(mix(q5mag, q5cyn, q5g1), q5cor, th1 * th1 * th1 * 0.6)
            let a5c2 = mix(mix(q5mag, q5cyn, q5g2), q5cor, th2 * th2 * th2 * 0.6)
            let a5c3 = mix(mix(q5mag, q5cyn, q5g3), q5cor, th3 * th3 * th3 * 0.6)
            let a5c4 = mix(mix(q5mag, q5cyn, q5g4), q5cor, th4 * th4 * th4 * 0.6)
            let a5c5 = mix(mix(q5mag, q5cyn, q5g5), q5cor, th5 * th5 * th5 * 0.6)
            let a5c6 = mix(mix(q5mag, q5cyn, q5g6), q5cor, th6 * th6 * th6 * 0.6)

            // ---- mode 6: ELECTRIC: cyan corona (accent) -> cold-white crack on
            // the thump. Flicker + violet spark dropped (too distracting). ----
            let q6wht = vec3(0.86, 0.96, 1.00)
            let a6lev1 = th1
            let a6lev2 = th2
            let a6lev3 = th3
            let a6lev4 = th4
            let a6lev5 = th5
            let a6lev6 = th6
            let q6r1 = clamp((th1 - 0.5) / 0.46, 0.0, 1.0)
            let q6r2 = clamp((th2 - 0.5) / 0.46, 0.0, 1.0)
            let q6r3 = clamp((th3 - 0.5) / 0.46, 0.0, 1.0)
            let q6r4 = clamp((th4 - 0.5) / 0.46, 0.0, 1.0)
            let q6r5 = clamp((th5 - 0.5) / 0.46, 0.0, 1.0)
            let q6r6 = clamp((th6 - 0.5) / 0.46, 0.0, 1.0)
            let a6c1 = mix(acc, q6wht, q6r1 * q6r1 * (3.0 - 2.0 * q6r1))
            let a6c2 = mix(acc, q6wht, q6r2 * q6r2 * (3.0 - 2.0 * q6r2))
            let a6c3 = mix(acc, q6wht, q6r3 * q6r3 * (3.0 - 2.0 * q6r3))
            let a6c4 = mix(acc, q6wht, q6r4 * q6r4 * (3.0 - 2.0 * q6r4))
            let a6c5 = mix(acc, q6wht, q6r5 * q6r5 * (3.0 - 2.0 * q6r5))
            let a6c6 = mix(acc, q6wht, q6r6 * q6r6 * (3.0 - 2.0 * q6r6))

            // -- per-mode selector weights (exactly one is 1; no `if` in DSL) --
            let s05 = sign(self.mode - 0.5)
            let s15 = sign(self.mode - 1.5)
            let s25 = sign(self.mode - 2.5)
            let s35 = sign(self.mode - 3.5)
            let s45 = sign(self.mode - 4.5)
            let s55 = sign(self.mode - 5.5)
            let s65 = sign(self.mode - 6.5)
            let s75 = sign(self.mode - 7.5)
            let m0 = 0.5 - 0.5 * s05
            let m1 = (0.5 + 0.5 * s05) * (0.5 - 0.5 * s15)
            let m2 = (0.5 + 0.5 * s15) * (0.5 - 0.5 * s25)
            let m3 = (0.5 + 0.5 * s25) * (0.5 - 0.5 * s35)
            let m4 = (0.5 + 0.5 * s35) * (0.5 - 0.5 * s45)
            let m5 = (0.5 + 0.5 * s45) * (0.5 - 0.5 * s55)
            let m6 = (0.5 + 0.5 * s55) * (0.5 - 0.5 * s65)
            let m7 = (0.5 + 0.5 * s65) * (0.5 - 0.5 * s75)
            let seq = m1 + m2 + m3

            // -- resolve per-segment level + target colour, then recolor --
            let lev1 = m0 * g1 + seq * bl1 + m4 * a4lev1 + m5 * a5lev1 + m6 * a6lev1 + m7 * th1
            let lev2 = m0 * g2 + seq * bl2 + m4 * a4lev2 + m5 * a5lev2 + m6 * a6lev2 + m7 * th2
            let lev3 = m0 * g3 + seq * bl3 + m4 * a4lev3 + m5 * a5lev3 + m6 * a6lev3 + m7 * th3
            let lev4 = m0 * g4 + seq * bl4 + m4 * a4lev4 + m5 * a5lev4 + m6 * a6lev4 + m7 * th4
            let lev5 = m0 * g5 + seq * bl5 + m4 * a4lev5 + m5 * a5lev5 + m6 * a6lev5 + m7 * th5
            let lev6 = m0 * g6 + seq * bl6 + m4 * a4lev6 + m5 * a5lev6 + m6 * a6lev6 + m7 * th6
            let tc1 = (m0 + m1) * acc + m2 * ce1 + m3 * pk1 + m4 * a4c1 + m5 * a5c1 + m6 * a6c1 + m7 * sg1
            let tc2 = (m0 + m1) * acc + m2 * ce2 + m3 * pk2 + m4 * a4c2 + m5 * a5c2 + m6 * a6c2 + m7 * sg2
            let tc3 = (m0 + m1) * acc + m2 * ce3 + m3 * pk3 + m4 * a4c3 + m5 * a5c3 + m6 * a6c3 + m7 * sg3
            let tc4 = (m0 + m1) * acc + m2 * ce4 + m3 * pk4 + m4 * a4c4 + m5 * a5c4 + m6 * a6c4 + m7 * sg4
            let tc5 = (m0 + m1) * acc + m2 * ce5 + m3 * pk5 + m4 * a4c5 + m5 * a5c5 + m6 * a6c5 + m7 * sg5
            let tc6 = (m0 + m1) * acc + m2 * ce6 + m3 * pk6 + m4 * a4c6 + m5 * a5c6 + m6 * a6c6 + m7 * sg6
            let kg1 = mix(k1, tc1, clamp(lev1, 0.0, 1.0))
            let kg2 = mix(k2, tc2, clamp(lev2, 0.0, 1.0))
            let kg3 = mix(k3, tc3, clamp(lev3, 0.0, 1.0))
            let kg4 = mix(k4, tc4, clamp(lev4, 0.0, 1.0))
            let kg5 = mix(k5, tc5, clamp(lev5, 0.0, 1.0))
            let kg6 = mix(k6, tc6, clamp(lev6, 0.0, 1.0))

            // ---- FPS-heat tint (mode-independent): flush every segment toward
            // the heat colour by `meter`. `meter` only rises while hover==0, so
            // the shimmer term (g1..g6) is already at rest here -- the two never
            // fight. `meter==0` leaves kg1..kg6 untouched (wordmark unchanged).
            let kg1 = mix(kg1, self.fps_color, self.meter)
            let kg2 = mix(kg2, self.fps_color, self.meter)
            let kg3 = mix(kg3, self.fps_color, self.meter)
            let kg4 = mix(kg4, self.fps_color, self.meter)
            let kg5 = mix(kg5, self.fps_color, self.meter)
            let kg6 = mix(kg6, self.fps_color, self.meter)

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

// FPS-heat meter ease-in/out duration (seconds): `meter` ramps 0->1 as metering
// engages, 1->0 as it releases (or on hover, which always suppresses heat).
const METER_SECS: f64 = 0.2;

// FPS smoothing time constant (seconds) for the exponential moving average --
// small enough to react to load, large enough to steady the per-frame jitter.
const FPS_TAU: f64 = 0.15;

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

    // Splash colour-pulse variant (1..7). `#[live]` so the start-screen sets the
    // initial variant; clicking the splash advances it (1->7, wrapping) and this
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
    // FPS-heat meter state (top-bar wordmark only; splash never meters). App
    // flips `metering` across a pointer interaction; the rest is owned here.
    // `metering` -- App's interaction-span flag (pointer press..release).
    #[rust]
    metering: bool,
    // Eased 0..1 heat strength fed to the shader `meter` uniform (ramps toward 1
    // while metering & not hovered, toward 0 otherwise) -- smooths enable/disable.
    #[rust]
    meter: f32,
    // EMA-smoothed framerate (Hz), sampled from next-frame dt while metering.
    #[rust]
    fps: f32,
    // Heat colour (green->amber->red) mapped from `fps`, pushed as `fps_color`.
    #[rust]
    fps_color: [f32; 3],
    // Set on a metering rising edge: skips the first fps sample so a stale dt
    // (idle gap before the press) can't flash red.
    #[rust]
    skip_fps_sample: bool,
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

                // FPS-heat meter, layered over the hover easing above. Hover
                // always suppresses heat, so target is 0 whenever hovered.
                let meter_target = if self.hovered {
                    0.0
                } else if self.metering {
                    1.0
                } else {
                    0.0
                };
                let mstep = (dt / METER_SECS) as f32;
                if self.meter < meter_target {
                    self.meter = (self.meter + mstep).min(meter_target);
                } else if self.meter > meter_target {
                    self.meter = (self.meter - mstep).max(meter_target);
                }
                // Sample framerate only while metering. Skip the first frame
                // after enable (its dt is a stale idle gap -> false red flash).
                if self.metering {
                    if self.skip_fps_sample {
                        self.skip_fps_sample = false;
                    } else {
                        let cdt = dt.clamp(0.001, 0.5);
                        let inst_fps = 1.0 / cdt;
                        let alpha = 1.0 - (-cdt / FPS_TAU).exp();
                        self.fps = (self.fps as f64 + alpha * (inst_fps - self.fps as f64)) as f32;
                        self.fps_color = Self::heat_color(self.fps);
                    }
                }

                // Keep the loop armed while hover OR the meter is live, so both
                // ease out; true idle (no hover, no meter) schedules no frames.
                if self.hovered || self.hover > 0.0 || self.metering || self.meter > 0.0 {
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
                    self.mode = self.mode % 7.0 + 1.0;
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
        // FPS-heat uniforms, every draw. Splash leaves `meter` at 0, so the tint
        // is a no-op there regardless of `fps_color`.
        self.draw_bg.set_uniform(cx, live_id!(fps_color), &self.fps_color);
        self.draw_bg.set_uniform(cx, live_id!(meter), &[self.meter]);
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

    /// Toggle framerate metering (called by `App` on pointer press/release).
    /// No-op on the splash (`auto`) -- it never meters. On a rising edge it
    /// primes the sampler: `last_time` is reset to now and the first fps sample
    /// is skipped (a stale idle-gap dt would flash red), then the frame loop is
    /// armed so the meter eases in and (on release) back out.
    pub fn set_frame_metering(&mut self, cx: &mut Cx, on: bool) {
        if self.auto {
            return;
        }
        if on && !self.metering {
            self.last_time = cx.seconds_since_app_start();
            self.skip_fps_sample = true;
        }
        self.metering = on;
        self.next_frame = cx.new_next_frame();
        self.draw_bg.redraw(cx);
    }

    /// Map a smoothed framerate (Hz) to a heat colour: green at >=60, amber at
    /// 30, red at <=15, lerped piecewise between. Playful, not calibrated.
    fn heat_color(fps: f32) -> [f32; 3] {
        const GREEN: [f32; 3] = [0.235, 0.745, 0.353];
        const AMBER: [f32; 3] = [0.902, 0.588, 0.078];
        const RED: [f32; 3] = [0.922, 0.275, 0.471];
        fn lerp(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
            let t = t.clamp(0.0, 1.0);
            [
                a[0] + (b[0] - a[0]) * t,
                a[1] + (b[1] - a[1]) * t,
                a[2] + (b[2] - a[2]) * t,
            ]
        }
        if fps >= 60.0 {
            GREEN
        } else if fps >= 30.0 {
            lerp(AMBER, GREEN, (fps - 30.0) / 30.0)
        } else if fps >= 15.0 {
            lerp(RED, AMBER, (fps - 15.0) / 15.0)
        } else {
            RED
        }
    }
}
