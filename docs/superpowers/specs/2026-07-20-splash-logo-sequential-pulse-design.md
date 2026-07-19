# Splash logo sequential colour pulse — design

**Date:** 2026-07-20 · **Branch:** `ui-tweaks-2` · **Status:** implemented (variants pending user pick)

## Goal

The splash screen (`start_screen.rs`) shows the WAML wordmark — a single
stylized "W" folded-ribbon of 6 SDF zigzag bars (`LogoMark`, `logo.rs`).
Make it **pulse in colour**, sweeping the overlapping "letters" the user reads
in the zigzag, in order **W → A → M → L**, looping, always-on. The top-bar
wordmark keeps its existing hover shimmer, untouched.

## Letter → segment map (the "major thump")

Bars are numbered left→right (seg1..seg6, x-centres ~0.16/0.32/0.47/0.62/0.75/0.89).
The letters are overlapping readings of the same ribbon:

| Letter | Segments |
|--------|----------|
| W | 1,2,3,4 |
| A | 2,3 |
| M | 2,3,4,5 |
| L | 5,6 |

A segment's pulse level = the max over the letters (currently lit) that contain
it. Each letter is a gaussian pulse centred in the loop. The letter beat is the
**major thump**; secondary low-amplitude **bleed + flicker** fill the idle bars
so the mark stays alive between beats (per user: "it can bleed and flicker
elsewhere, but this would be the major thump").

## Architecture

Branchless, single-shader, mode-selected (the DSL compiles no `if`).

1. **`logo.rs` shader** — new `mode: uniform(0.0)`.
   - `mode 0` = existing hover shimmer (top-bar wordmark) — byte-identical path.
   - `modes 1..6` = always-on splash letter pulse; all modes' maths computed,
     then selected by per-mode weights `m0..m6` (exactly one is 1, via `sign`).
   - Shared thump levels `th1..th6` + bleed/flicker feed all pulse modes; each
     mode differs only in target colour.
2. **`LogoMark` widget** — new `auto: bool` live-prop. When true the widget
   free-runs its NextFrame loop (no hover gate), feeds `time`, and skips the
   cursor/click behaviour. Re-armed every `draw_walk` so hide/show resumes
   cleanly (NextFrame tokens overwrite — no stacking).
3. **`start_screen.rs`** — splash logo becomes the `LogoMark` widget with
   `auto: true` + a chosen `mode` (default `2`, Close Encounters).
4. **`bin/logo_pulse_harness.rs`** — all 6 variants stacked on the splash-light
   ground, each animating, for side-by-side comparison + screenshots.

## The six colour variants

Same W→A→M→L thump geometry; colour scheme differs:

1. **accent** — single `#1496dc`, only the active letter's bars lit.
2. **close encounters** — Spielberg light-organ: per-letter hue
   (red/orange/green/violet), blended per segment by each contributing letter's
   live level; idle settles to accent.
3. **bucket palette** — our 7-swatch `AccentBucket` palette
   (`inspector_panel.rs`); v1 fixes 4 of 7 (Interface blue / UseCase amber /
   Package green / Behavior pink). *Tunable:* rotate the 4-of-7 window per loop
   so all 7 surface over time.
4. **molten** — viscous magenta→violet→teal plasma, hot-gold core on the thump
   (agent-authored, "Molten Wordmark").
5. **neon** — synthwave magenta↔cyan chroma sweep, white-hot strike, VHS
   flicker (agent-authored, "Neon Sign Ignition").
6. **electric** — cyan corona → cold-white crack, current arcs bar-to-bar,
   violet sparks (agent-authored, "Capacitor Crack").

## Timing (first-pass, screenshot-tune later)

Full W→A→M→L loop ~3.2 s incl. a short pause before wrap. Gaussian letter width
`wdt = 0.014`. Bleed breathe ~0.05, per-bar flicker ~0.12 (both kept under the
thump). Agent variants carry their own tunables (documented inline in `logo.rs`).

## Non-goals

- No new letterform geometry (the mark stays the 6-bar W ribbon).
- Top-bar wordmark behaviour is unchanged (mode 0).
- Final variant + exact colours/timing are chosen after the user reviews the
  harness screenshots.
