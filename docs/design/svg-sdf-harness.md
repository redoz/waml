# SVG-vs-SDF Glyph Harness Spec

Dev harness (`crates/waml-editor/src/bin/logo_harness.rs`) to eyeball an
`icons.rs` SDF glyph against its Lucide SVG source. **Not shipped.** Parked
under the radial-command-menu branch; icons.rs + logo_harness.rs are baseline
(off the radial feature scope) — committing them is a separate decision.

## Subject

**ONE icon: `pin`.** `pin-off` is a different icon — out of scope here.
- SVG source: `crates/waml-editor/resources/icons/pin.svg` (Lucide, 24x24
  viewBox, stroke-based). Currently untracked (restored from commit 49afe0e).
- SDF: `icons.rs` `IconPin` (`mod.draw.IconPin`, drawn via `TreeIcons.pin`).

## Layout — 3 columns

Each column shows the SAME glyph at sizes **56, 24, 18, 16, 14 px**, stacked
vertically, top-aligned, same row baselines across all three columns.

| Col | Content | Background |
|-----|---------|-----------|
| A | SVG reference (`DrawSvg` / `Icon`, `pin.svg`) | transparency checkerboard |
| B | SDF port (`IconPin`) | transparency checkerboard |
| C | Overlay diff (impartial judge) | dark (for additive read) |

### Checkerboard
Standard transparency checkerboard behind A + B — light/dark gray squares,
fixed cell size (independent of glyph size) so the two columns share one grid.

### Overlay diff (Col C)
Per row/size: draw SVG tinted **red**, SDF tinted **blue**, **additive** blend.
- purple/magenta = pixels agree
- red only = SVG has coverage, SDF missing
- blue only = SDF has coverage, SVG missing

Additive needs a dark bg to read; Col C is dark, not checkerboard.

## Open technical questions
- Additive blend path in makepad: DrawSvg + DrawColor into same cell with
  additive blending. If per-draw blend mode isn't exposed, fall back to a
  custom DrawQuad that samples both, or render red/blue at partial alpha and
  accept approximate overlap.
- SDF glyphs authored in local `rect_size` — confirm they hold at 14px.

## Deferred
- CPU numeric diff (rasterize both to same buffer, subtract, print a mismatch
  score) — a true numeric impartial judge. Overlay chosen for now (no deps,
  immediate). Hook left if a number is wanted later.

## Run loop (no hot-reload in bare cargo run)
1. `cargo build -p waml-editor --bin logo_harness`
2. launch `target/debug/logo_harness.exe` as bg task
3. wait ~8s, capture via `scratchpad/cap.ps1` (PrintWindow, run via
   `powershell.exe` — WinPS5 has System.Drawing, pwsh7 does not)
4. Read the PNG, judge, edit `icons.rs` pin vertices, repeat
- Shader errors surface at GPU runtime in stdout as `[E] ...icons.rs:LINE`,
  NOT at cargo build. Grep the log.
- SILENT degeneracy: path strokes blank if a vertex < ~0.10 off viewport edge;
  `sdf.box(..,0.0)` degenerates+floods — use `sdf.rect`. Keep pin vertices
  >= 0.11 margin.
