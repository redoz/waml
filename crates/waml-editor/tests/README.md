# waml-editor tests

`waml-editor` is a library crate with a thin `main.rs` shim (it just calls
`app_main!(App)`): every module is declared in `src/lib.rs`, so the inline
`#[cfg(test)]` unit tests in `src/*.rs` run as the **lib** unit-test harness.
The five always-enabled integration files in `tests/` (`editor_history.rs`,
`view_history.rs`, `history_integration.rs`, `markdown_authority.rs`,
`markdown_integration.rs`) link that same compiled library and test its public
modules (`editor_history`, `view_history`) plus filesystem-level fixtures. The
feature-gated `ui.rs` target runs the semantic editor journey described below.
Modules stay crate-private (`mod`) unless a `tests/` file or a `src/bin/*`
harness actually imports them.

## Unit tests (no GPU)

```bash
cargo test -p waml-editor
```

Covers the engine-agnostic modules: `load`, `sizing`, `scene` (including routed
edge polylines), `camera`, `cli`, `tree::build_tree`, and `tree_panel`'s id-map
round-trip. No GPU required.

## Semantic editor navigation

Linux runs the semantic journey headlessly:

```bash
rtk cargo test -p waml-editor --features ui-tests --test ui -- --test-threads=1
```

The feature gate keeps this target out of the normal
`cargo nextest run --workspace --profile ci` suite. CI runs the dedicated
command once, after the normal workspace tests, in the required Linux
`build-test` job. The journey verifies that the staged Mini fixture is ready,
activates the Orders diagram, and switches the active document from Diagram to
Source and back to Diagram.

Windows does not have a working headless runtime for this journey. Do not run
the native UI test binary headlessly on Windows. Windows CI still compiles all
feature-gated code through
`cargo clippy --workspace --all-targets --all-features -- -D warnings`. A
Windows developer can compile the target without running it:

```powershell
rtk cargo test -p waml-editor --features ui-tests --test ui --no-run
```

For visible diagnosis, start Makepad Studio and its remote bridge first. Then
run the same scenario without source changes:

```powershell
$env:MAKEPAD_TEST_VISIBLE='1'
$env:MAKEPAD_TEST_STUDIO='127.0.0.1:8001'
$env:MAKEPAD_TEST_STUDIO_MOUNT='waml'
rtk cargo test -p waml-editor --features ui-tests --test ui -- --test-threads=1
```

Scenario files are a typed semantic DSL. Import only semantic types from
`waml_ui_test`, and call `WamlApp` domain operations. Do not import
`makepad_test` or use selectors, widget IDs, coordinates, sleeps, raw Makepad
events, or timeout values. An `ensure_*` operation establishes an idempotent
precondition, an imperative operation performs one action, and an `expect_*`
operation observes without mutation.

A failed run prints and preserves its directory under
`target/waml-ui-test/<run-id>/<test-slug>/`. The directory contains:

```text
semantic-trace.txt
semantic-trace.json
failure.txt
logs.txt
widget-snapshot.json
widget-tree.txt
failure-screenshot.png
workspace/
```

The `workspace/` directory is the staged, run-owned fixture. Successful runs
remove their run directory.

The automated journey is the verification of record for fixture readiness,
Orders activation, and Diagram/Source switching. Manual verification remains
the record for visual rendering, temporal canvas gestures, and navigation that
the semantic journey does not cover. You can perform a covered navigation step
to set up a manual visual check, but do not record that setup step as separate
manual verification.

## Manual visual and interaction verification (verification of record)

```powershell
./run.ps1 -Title manual-visual-check
```

Committed fixtures under `tests/fixtures/` are **read-only inputs**: the
editor writes layout back into the loaded bundle, so always launch a staged
copy (`run.ps1` does this automatically for any fixture under
`tests/fixtures/`; a bare `cargo run -p waml-editor -- <fixture>` does not,
and will dirty the committed files).

Opens the native GPU window. The window is a resizable `Splitter`: the left
pane is the `ProjectTree` panel (a `FileTree` showing the `Mini` bundle's root
package with the `Order`/`Customer` classifiers and the `Orders` diagram); the
right pane is the `ClassDiagramSurface`. Pan the canvas with left-drag, zoom
with the scroll wheel, and drag the splitter bar to resize the panes. This
interactive run is the verification of record for renderer output, tree
presentation, temporal gestures, and uncovered navigation. It is not the
verification of record for fixture load, Orders activation, or Diagram/Source
switching. There is no automated headless pixel-render check (see below).

### Class-diagram surface regression pass

Build and launch the executable from the worktree-local target directory, then
capture only the PID returned by that launch:

```powershell
rtk cargo build -p waml-editor --target-dir target
$editor = Start-Process -FilePath target\debug\waml-editor.exe `
  -ArgumentList crates/waml-editor/tests/fixtures/mini -PassThru
rtk pwsh -File scripts/capture-window.ps1 -Out C:\tmp\class-surface-after-mini.png `
  -ProcessId $editor.Id
```

Repeat with the `groups` and `sixkind` fixtures to record these seven comparison
states: `mini`, `groups`, `groups-hidden`, `sixkind-overview`,
`sixkind-zoomed-out`, `sixkind-zoomed-in`, and `sixkind-focus`. Compare each
against its matching baseline at native resolution. Check group and nested-group
bounds, routed edges and terminal adornments, large/expanded cards, selection,
constraint/conflict focus, hidden borders, and both font raster levels.

The screenshots are not a substitute for temporal interaction verification.
In one native session, exercise pan, wheel zoom, pinch, scene/selection fits,
selection/deselection, expansion, inspector selection, context menu, the full
drag/dwell/retarget/preview/commit/cancel flow, scene refresh with camera
retention, conflict focus and revalidation, and tab/scene changes during dwell
or preview. Confirm no stale timer, dial, selection index, preview layout, or
camera animation survives cancellation or a scene change.

### Editor ownership parity

Capture these fixed-size native screenshots before and after the ownership
refactor: `start-screen`, `class-diagram`, `classifier-preview`, `source-view`,
`tab-switching`, `popup`, `overlay`, and `docks-closed`.

Interaction checklist: open/replace/promote/activate/close tabs; close fallback;
picker and placement-dial armed/closed order;
conflict focus, delete, keep-open and dismiss; burger/logo/node/nav/doc-switcher
popups; shortcuts/fonts/icons/colors overlays; wide/narrow left and right dock
toggles; browser debounce save and refresh restore; native save remains
non-durable.

## Headless render regression check — intentionally absent

Task 9 investigated producing a headless PNG of the fixture for eyeball review
and future regression. **No headless integration test was written**, for two
independent, decisive reasons found while implementing it:

1. **The fork's headless backend does not compile on Windows** (the development
   / target platform). The vendored makepad *does* ship a headless CPU renderer
   under `C:\dev\vendor\makepad\platform\src\os\headless\` (a JIT-shader +
   software rasterizer in `raster.rs` / `virtual_gpu.rs` that encodes PNGs via
   `encode_png_rgba`). It is gated behind a **compile-time cfg**, not a Cargo
   feature: `build.rs` turns the `MAKEPAD=headless` env var into
   `rustc-cfg=headless`, which swaps out the entire OS backend
   (`platform/src/os/mod.rs`). Building `waml-editor` with `MAKEPAD=headless`
   fails to compile `makepad-platform` with 14 errors — e.g.
   `gl_render_bridge.rs` / `cx_api.rs` unconditionally reference
   `os::windows::…` and `CxOs::d3d11_device` (both `#[cfg(not(headless))]`
   only), and the Windows `HeadlessLoadedModule` JIT stub is missing the
   `symbol` method that `raster.rs` / `shader.rs` call. The headless path is
   only wired up for non-Windows hosts in this fork.

2. **Even where it builds, it is not reachable as an integration test.** The
   headless renderer is a *whole-app, separate-build-configuration* mechanism:
   you build the entire binary with `MAKEPAD=headless` and run it, and the
   headless event loop (`Cx::event_loop` → `headless_single_frame`) renders the
   real draw tree and writes `window_0_frame_000000.png` to
   `MAKEPAD_HEADLESS_OUT_DIR`. The rendering entry points
   (`Cx::headless_render_all_passes`, `encode_png_rgba`) are `pub(crate)` and
   `#[cfg(headless)]` inside `makepad-platform` — not a public API and not even
   compiled in a normal `cargo test` build. A `tests/*.rs` integration test is a
   *separate crate* that can only touch `waml-editor`'s public items, and
   `ClassDiagramSurface` is a **crate-private** widget (its module is declared
   as a plain `mod` in `lib.rs`).
   There is no in-process "render this widget to an RGBA buffer" function to
   call, so the check cannot participate in `cargo test -p waml-editor`.

Because the pixel renderer is platform-incomplete here **and** structurally
unreachable from an external test crate, the automated headless pixel test is
omitted (a plan-sanctioned outcome). The semantic Linux journey covers project
readiness, Orders activation, and Diagram/Source switching, but it does not
qualify pixel output or temporal gestures. The interactive `cargo run` above
remains the verification of record for those visual and temporal properties,
the `ProjectTree` fold state, and navigation outside the semantic journey.
The data-layer pieces — `tree::build_tree` and the `tree_panel` id-map
round-trip — remain unit-tested above. If the fork later fixes the Windows
headless pixel backend, the manual regression flow would be:

```bash
# (only works once the fork's Windows headless backend compiles)
MAKEPAD=headless MAKEPAD_HEADLESS_OUT_DIR=<out-dir> \
  cargo run -p waml-editor -- crates/waml-editor/tests/fixtures/mini
# -> writes <out-dir>/window_0_frame_000000.png for eyeball review
```

## Markdown presentation and motion

The native harness presents a fixed `1280 x 900` logical window for each source
and motion state. Build it in release mode, then capture only the PID created for
the current case. A case-specific ready marker is written after the final redraw
has been presented. The capture waits for both that marker and the launched
process window; a nonzero window handle alone is not the readiness contract.

Store the evidence outside the repository at
`C:\tmp\markdown-presentation-verification`:

```powershell
rtk cargo build -p waml-editor --bin markdown_presentation_harness --release
rtk proxy pwsh -NoProfile -Command '$out = "C:\tmp\markdown-presentation-verification"; New-Item -ItemType Directory -Force -Path $out | Out-Null; $cases = @("headings","inline","lists","quotes","code","tables","images","invalid","selection","motion-start","motion-mid","motion-end"); foreach ($case in $cases) { $ready = "$out\$case.ready"; Remove-Item -LiteralPath $ready -ErrorAction SilentlyContinue; $p = Start-Process -FilePath "target\release\markdown_presentation_harness.exe" -ArgumentList @("--case",$case) -PassThru; try { $deadline = [DateTime]::UtcNow.AddSeconds(20); while (($p.MainWindowHandle -eq 0 -or -not (Test-Path -LiteralPath $ready)) -and [DateTime]::UtcNow -lt $deadline) { Start-Sleep -Milliseconds 100; $p.Refresh() }; if ($p.MainWindowHandle -eq 0) { throw "window did not open for $case" }; if (-not (Test-Path -LiteralPath $ready)) { throw "final frame was not presented for $case" }; & pwsh -NoProfile -File scripts/capture-window.ps1 -Out "$out\$case.png" -ProcessId $p.Id; if ($LASTEXITCODE -ne 0) { throw "capture failed for $case" } } finally { Stop-Process -Id $p.Id -ErrorAction SilentlyContinue } }'
```

Expect these twelve native-pixel PNGs: `headings`, `inline`, `lists`, `quotes`,
`code`, `tables`, `images`, `invalid`, `selection`, `motion-start`,
`motion-mid`, and `motion-end`. `PrintWindow` captures `1280 x 900` logical
window content at the host native DPI. The workflow never finds, reuses, or
stops a process by name.

The ready marker is valid only after the opt-in native paint generation has
advanced for the final requested draw. No redraw is pending after the marker.
For endpoint comparison, set `WAML_MARKDOWN_HARNESS_TARGET_ONLY=1` only for a
second `motion-end` launch and capture it as `motion-end-target.png`. This mode
installs the same incremental target without interpolation and is not an extra
matrix case. Compare decoded pixels with `motion-end.png`. Require nonempty and
equal foreground-pixel counts in the heading, insertion, stable paragraph, and
image-source regions. Permit at most a one-value RGB raster quantization delta;
do not permit a changed layout or a missing region.

Inspect every image and record these checks:

- All literal delimiters stay visible. Markers have lower contrast, and active
  markers keep the same geometry.
- Heading hierarchy is moderate. The document inset is 24 logical pixels on
  the left, right, top, and bottom. Lists hang from their literal markers.
- Quote, code, table, checkbox, thematic-rule, and image decorations do not
  replace source. Raw HTML stays visible and inert.
- The three image source lines remain visible in loading, failed/retry, and
  approved `checker.svg` byte states.
- Motion start, midpoint, and end keep the same surviving identities. The
  midpoint is 87.5% of displacement because `OutCubic(0.5) = 0.875`.
- Selection, diagnostics, image geometry, caret, and IME stay attached to their
  source text at each applicable sampled phase.

## Markdown editor integration rollout

The native verification fixture is
`crates/waml-editor/tests/fixtures/markdown-integration`. It contains a loadable
`index.md`, the source document `evidence.md`, and its local `evidence.svg`.
`waml list` loads the `evidence` `uml.Class`, while `waml check` reports exactly
the intended recoverable `unterminated multiplicity` diagnostic.

The verification-of-record capture is
`target/markdown-editor-integration.png`. It is generated evidence and is not
committed. The successful capture used commit `2d23c7e9`, Windows DPI 96
(scale 1.0), a `1280 x 1200` native-pixel window, and the default reduced-motion
state (not forced). The editor needs approximately 10 seconds to present the
populated first frame on this machine.

The successful launch used a byte-for-byte staging copy of the committed
fixture at `target/task9-native-fixture`:

```powershell
rtk cargo build -p waml-editor --bin waml-editor
rtk pwsh -NoProfile -Command '$p = Start-Process -FilePath "target\debug\waml-editor.exe" -ArgumentList "target\task9-native-fixture" -PassThru; $p.Id'
```

That staging directory contained only the committed fixture's `index.md`,
`evidence.md`, and `evidence.svg`. The fixture is now directly launchable with
the same binary. There is no production CLI flag for SourceView, so the native
flow used the Project Tree context menu's **View Source** action. It then resized
the exact launched window, made a deterministic text selection, and captured
only that PID:

```powershell
rtk pwsh -File scripts/capture-window.ps1 `
  -Out target/markdown-editor-integration.png -ProcessId <launched-pid>
```

The `1280 x 1200` PNG visibly contains all required cases: the complete
mixed-metric heading with literal `#`, `**`, and backticks; a real blue
selection and caret; the image source plus explicit SVG placeholder; a red
diagnostic underline; a complete fenced `waml` block with highlighted `String`;
and no canvas bleed-through.

Final verification before integration:

- `cargo test -p waml-editor --test markdown_authority`: 5 passed.
- `cargo test -p waml-markdown-editor`: 220 passed.
- `cargo test -p waml-editor`: 1001 passed, 4 ignored.
- `cargo test -p waml-cli`: 84 passed.
- `cargo fmt --all -- --check`: passed.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`:
  0 errors; only the two existing duplicate-package Cargo notices.
- `cargo test --workspace`: green, unfiltered.
- `editors/vscode`: `pnpm run build` passed, `pnpm test` passed 14 tests,
  and `pnpm run lint` passed.

The `waml-syntax` incremental property defect that once forced a `--skip`
filter on `randomized_full_and_incremental_snapshots_agree` and
`valid_edit_sequences_match_full_parse` (reparse windows swallowing trailing
end-of-file whitespace) was fixed in commit `10f66dc9`. Run the workspace gate
unfiltered — those two property tests are the ones that catch this crate's
hardest bug class, and a red there is a *new* defect, not the documented one.
