# waml-editor tests

`waml-editor` is a **binary-only** crate (no `lib.rs`), so its unit tests live
inline in `src/*.rs` behind `#[cfg(test)]` and run as the bin's unit-test
harness. There is no `--lib` target.

## Unit tests (no GPU)

```bash
cargo test -p waml-editor
```

Covers the engine-agnostic modules: `load`, `sizing`, `scene` (including routed
edge polylines), `camera`, `cli`, `tree::build_tree`, and `tree_panel`'s id-map
round-trip. No GPU required.

## Visual verification (verification of record)

```bash
cargo run -p waml-editor -- crates/waml-editor/tests/fixtures/mini
```

Opens the native GPU window. The window is a resizable `Splitter`: the left
pane is the `ProjectTree` panel (a `FileTree` showing the `Mini` bundle's root
package with the `Order`/`Customer` classifiers and the `Orders` diagram); the
right pane is the `ClassDiagramSurface`. Clicking the `Orders` diagram row loads it into
the canvas (fits on first draw). Pan the canvas with left-drag, zoom with the
scroll wheel; drag the splitter bar to resize the panes. This interactive run is
the **verification of record** for both the renderer and the tree panel — there
is no automated headless render check (see below).

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
   `ClassDiagramSurface` is a **bin-private** widget (declared via `mod` in `main.rs`).
   There is no in-process "render this widget to an RGBA buffer" function to
   call, so the check cannot participate in `cargo test -p waml-editor`.

Because the headless backend is platform-incomplete here **and** structurally
unreachable from an external test crate, the automated headless test is omitted
(a plan-sanctioned outcome). The interactive `cargo run` above is the
verification of record — this applies equally to the `ProjectTree` panel added
in Task 3: it too is a bin-private widget with no in-process render hook, so
its `FileTree` rendering, fold state, and diagram-row click wiring are only
exercised by the same interactive run (its data-layer pieces — `tree::build_tree`
and the `tree_panel` id-map round-trip — remain unit-tested above). If the fork
later fixes the Windows headless backend, the manual regression flow would be:

```bash
# (only works once the fork's Windows headless backend compiles)
MAKEPAD=headless MAKEPAD_HEADLESS_OUT_DIR=<out-dir> \
  cargo run -p waml-editor -- crates/waml-editor/tests/fixtures/mini
# -> writes <out-dir>/window_0_frame_000000.png for eyeball review
```
