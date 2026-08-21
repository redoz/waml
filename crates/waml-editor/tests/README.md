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
`build-test` job.

The scenarios verify, against a real headless editor:

| Scenario | What it settles |
|---|---|
| `open_and_switch_document_views` | The staged Mini fixture is ready, Orders activates, and the active document switches Diagram -> Source -> Diagram. |
| `project_tree_lists_every_row_of_the_bundle` | The tree's exact row list, and its layout invariant (no zero-height, no overlap). A projection that silently drops a row fails here. |
| `opening_a_diagram_selects_its_row_in_view` | Opening a diagram selects its row **and leaves it inside the viewport**. |
| `palette_blends_a_query_into_titled_sections` | Ctrl+K blends one query into its titled sections with the right row counts. |
| `escalating_a_query_groups_results_by_document` | The results tab groups every hit by document, in rank order. |
| `find_strip_counts_hits_scoped_to_the_active_document` | Ctrl+F narrows the same query to the active tab's own document. |
| `f3_walks_the_find_hits_and_wraps_at_both_ends` | F3/Shift+F3 walk the find cursor and wrap at both ends. |
| `a_route_across_surfaces_leaves_exactly_one_of_them_showing` | A route crossing the surface boundary three times, with the centre held to **exactly one** surface at every stop. The siblings half of `show_*` has failed silently before. |
| `committing_a_hit_opens_its_document_and_selects_its_tree_row` | Committing the palette's top hit opens the right document, on the right surface for its kind, with its tree row selected. |
| `the_light_cycle_canvas_is_drawn_the_way_its_reference_was` | **The rendering gate.** The behavior canvas is drawn the way its stored reference was -- see below. |
| `the_orders_canvas_is_drawn_the_way_its_reference_was` | **The rendering gate, class canvas.** Class edges, card borders and compartment rules -- the half of ledger row V1 the behavior canvas cannot see. |

`waml_ui_test`'s crate docs carry the standing list of what this harness can
and cannot decide -- read them before adding a scenario for something that is
really a question about pixels.

## The rendering gate

Two scenarios look at pixels, one per canvas kind:

* `the_light_cycle_canvas_is_drawn_the_way_its_reference_was` opens a state
  machine whose `Active` node carries both a self-loop and a long back edge
  -- the two connectors `90ffcf0f` moved.
* `the_orders_canvas_is_drawn_the_way_its_reference_was` opens the `Mini`
  bundle's class diagram, which draws none of those: what it draws is a
  class association edge, three class cards with their compartment rules, an
  abstract title and a stereotype. Diagram pens (visual sign-off ledger V1)
  moved class edges 3.0 -> 2.0 deliberately, and an ink mask is exactly the
  instrument for a stroke that quantises to a different number of device
  pixels.

Each screenshots the headless window, crops to the diagram surface's own
rect, and compares against
`crates/waml-editor/tests/references/<name>.<os>-<arch>.ink`.

It compares **ink, not pixel values**: each pixel reduces to "background or
not", and the stored reference is a run-length-encoded mask in plain text
rather than a PNG. `waml_ui_test`'s `reference` module carries the argument
for both choices -- the short version is that antialias ramps, JIT-compiled
shaders, per-zoom text rasterisation and the pen quantiser all move pixel
values without moving whether a pixel has ink, and the headless PNG encoder
does not deflate, so a stored capture would be ~9 MB of undiffable binary.

**Linux is the platform of record, and not by preference.** The fork's
headless shader loader is `#[cfg(unix)]`; on Windows every shader compiles to
a `.dll` under `target/makepad-headless-jit/` and none of them loads, so the
virtual GPU never draws and the capture is a flat rectangle. (That is also
why a Windows run's preserved `failure-screenshot.png` has never been worth
opening.) The gate detects a blank capture and reports it rather than passing
quietly; on Windows, with no reference committed, it reports itself as not
run and the other scenarios are unaffected.

To accept an intended rendering change, or to record the first reference for
a platform:

```bash
WAML_UI_TEST_UPDATE_REFERENCES=1 \
  cargo test -p waml-editor --features ui-tests --test ui -- --test-threads=1
```

A blank capture is refused rather than recorded, so this cannot write a
reference that would pass forever.

In CI the gate runs inside the existing Linux `Semantic editor UI test` step,
and `Upload rendering gate evidence` carries its output off the runner in
both directions: on failure the capture, a red/green overlay of what moved
(red is ink that vanished, green is ink that appeared) and the mask that
would become the new reference; on success any reference recorded because
none existed for the platform yet. Downloading that artifact and committing
`recorded-references/*.ink` is what turns the gate from advisory to enforcing
on Linux.

**`orders` is waiting on exactly that.** `light-cycle.linux-x86_64.ink` is
committed and enforcing; the class canvas was added from a Windows machine,
which cannot record a Linux reference, so `orders.linux-x86_64.ink` still
has to be downloaded from the first Linux run's artifact and committed.
Until it is, that gate records and passes advisory, and says so in its
trace.

**Windows can run this now.** The fork's headless backend stopped being
macOS-only when the CI headless fix landed, and the whole suite has been run
green on Windows against fork rev `6534634a` (12 passed, one editor process
spawned per scenario) -- with both rendering gates reporting themselves
advisory, for the reason above. Budget generously: a scenario costs about
2-3 minutes on an idle machine and was measured at 4-5 with other builds
competing for the box, and the driver's startup budget is 600s with two
retries, so a genuinely wedged app takes 20 minutes to say so. A scenario
that has produced no `semantic-trace.txt` after two minutes is usually still
starting, not stuck.

Prebuild the exact configuration the driver spawns first, or the in-test
build's swallowed output turns a compile error into an unreadable startup
timeout:

```powershell
$env:MAKEPAD='headless'
$env:CARGO_TARGET_DIR='crates/waml-editor/target'
cargo build -p waml-editor --release
Remove-Item Env:CARGO_TARGET_DIR
rtk cargo test -p waml-editor --features ui-tests --test ui -- --test-threads=1
```

Linux CI remains the verification of record -- that is where the required
`build-test` job runs it -- but a Windows developer no longer has to push to
find out whether a scenario passes. Windows CI additionally compiles all
feature-gated code through
`cargo clippy --workspace --all-targets --all-features -- -D warnings`, and
the target can still be compiled without running it:

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
`target/waml-ui-test/<run-id>/<test-slug>/`. A controlled semantic failure
after launch and successful evidence capture is expected to contain:

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

Capture failures can produce `failure-screenshot-error.txt`,
`widget-tree-error.txt`, `widget-snapshot-error.txt`, or `logs-error.txt`
with the evidence that explains the corresponding capture failure. A failure
before application launch or before evidence capture completes can preserve
only the artifacts that the run produced before it failed. The controlled-red
set above is not unconditional for these earlier or capture-failure cases.

When present, the `workspace/` directory is the staged, run-owned fixture.
Successful runs remove their run directory.

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

### Use-case diagram baselines (manual, native)

**This is not the rendering gate** -- see "The rendering gate" above for the
automated one. This is the older manual tool, and it is kept for the one thing
the headless gate structurally cannot do: look at what the REAL D3D11
renderer draws. It needs a desktop session, launches the app headed, sleeps
15s per diagram for the GPU scene to settle, and byte-compares whole windows,
so it can only ever be run by hand on a native Windows desktop. That is why no
workflow calls it and why none should.

The `screenshots/use-case` directory contains native, HiDPI-correct captures of
the three shipped use-case views. The check launches each exact diagram with a
unique title, captures its child process, and compares dimensions and pixels:

```powershell
rtk pwsh -File scripts/check-use-case-diagram-screenshots.ps1
```

The default changed-pixel limit is `0.001`. To accept an intentional visual
change, review a native run first, then replace only the three manifest-owned
baselines with `-Update`. A missing source, wrong title declaration, missing
baseline, dimension change, or excessive pixel difference fails the check.
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
rtk cargo build -p waml-editor --features harness --bin markdown_presentation_harness --release
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
