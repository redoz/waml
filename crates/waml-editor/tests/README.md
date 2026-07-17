# waml-editor tests

`waml-editor` is a **binary-only** crate (no `lib.rs`), so its unit tests live
inline in `src/*.rs` behind `#[cfg(test)]` and run as the bin's unit-test
harness. There is no `--lib` target.

## Unit tests (no GPU)

```bash
cargo test -p waml-editor
```

Covers the engine-agnostic modules: `load`, `sizing`, `scene`, `camera`, `cli`,
the `canvas::border_point` geometry helper, `tree::build_tree`, and
`tree_panel`'s id-map round-trip. 32 tests, no GPU required.

## Visual verification (verification of record)

```bash
cargo run -p waml-editor -- crates/waml-editor/tests/fixtures/mini
```

Opens the native GPU window. The window is a resizable `Splitter`: the left
pane is the `ProjectTree` panel (a `FileTree` showing the `Mini` bundle's root
package with the `Order`/`Customer` classifiers and the `Orders` diagram); the
right pane is the `GraphCanvas`. Clicking the `Orders` diagram row loads it into
the canvas (fits on first draw). Pan the canvas with left-drag, zoom with the
scroll wheel; drag the splitter bar to resize the panes. This interactive run is
the **verification of record** for both the renderer and the tree panel — there
is no automated headless render check (see below).

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
   `GraphCanvas` is a **bin-private** widget (declared via `mod` in `main.rs`).
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
