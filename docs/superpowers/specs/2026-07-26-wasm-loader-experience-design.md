# WebAssembly Loader Experience

## Goal

Make the hosted editor's pre-WASM loading screen feel active from download
through first render without presenting invented progress.

The six logo segments must illuminate one at a time from left to right during
the byte-counted download. Once the download is complete, the loader must
communicate the remaining compilation and startup phases until the first app
frame is ready, then crossfade into that frame.

## Current behavior

`scripts/inject-runtime-shell.mjs` replaces cargo-makepad's generated
`.canvas_loader` contents. It draws a dim logo beneath a colored copy clipped by
one rectangular reveal. Increasing the rectangle width exposes vertical slices
through several diagonal polygons, so the logo does not read as six discrete
segments.

The runtime wrapper counts decoded WASM response bytes and stops changing the
loader at 100%. Makepad then compiles, instantiates, and starts the module before
removing `.canvas_loader` after it observes a presented frame. Those phases have
no current visual treatment, which makes the completed loader appear frozen.

## Design

### Markup and segment progress

The injected SVG will contain a dim ghost copy of the logo and a colored
progress copy whose six polygons are individually addressable in their existing
left-to-right source order.

Download progress remains derived from decoded bytes divided by the
build-time, decompressed WASM size. For progress `p`, segment `i` receives a
level equivalent to `clamp(p * 6 - i, 0, 1)`. This makes one polygon fade from
dim to full color before the next begins, rather than moving a vertical clip
through the whole mark. Progress remains monotonic.

### Runtime phases

The runtime shell owns a small loader state machine:

1. **Loading** begins at page load and updates segment levels from streamed byte
   progress.
2. **Compiling** begins visibly once the response stream reaches 100%. The
   segments are fully available and enter a restrained left-to-right chase.
   `WebAssembly.compileStreaming` begins earlier and overlaps the download, so
   this label specifically means "download complete; compilation still
   pending."
3. **Starting** begins when the `WebAssembly.compileStreaming` promise resolves.
   The same chase continues while Makepad instantiates the module, initializes
   the Rust application, and presents its first frame.
4. **Ready** is triggered by Makepad removing `.canvas_loader`, which already
   occurs only after its presented-frame checks or fallback.

The labels are `Loading…`, `Compiling…`, and `Starting…`. No percentage is shown
because compilation and startup expose completion boundaries but no meaningful
fractional progress.

The runtime will wrap `WebAssembly.compileStreaming` before cargo-makepad's
module script runs. The wrapper preserves its arguments, receiver, returned
promise, and rejection behavior; it only updates the loader when the promise
settles.

### First-frame crossfade

A `MutationObserver` installed by the classic runtime script will watch for
Makepad removing `.canvas_loader`. On the first removal, it will immediately
reattach that same overlay for presentation only, mark it as fading, and remove
it permanently after an approximately 250 ms opacity transition.

Mutation observers run before the browser paints the removal, so the rendered
app appears beneath a continuous overlay rather than through a one-frame flash.
The observer must distinguish Makepad's first removal from its own final cleanup
to prevent reattachment loops. The canvas remains untouched and ready beneath
the fading overlay.

### Motion and appearance

The compile/start chase lights neighboring segments in left-to-right order and
loops gently; it does not reset download progress or imply a percentage.
Status text sits beneath the mark using the existing system font and subdued
foreground color.

Under `prefers-reduced-motion: reduce`, the logo stays fully lit during
compilation/startup and the final fade is shortened or removed. The phase label
still changes, so state is not communicated by motion alone.

### Failure behavior

If `compileStreaming` rejects, the loader stops chasing and displays
`Couldn’t start WAML`. The original rejection is rethrown unchanged so existing
console diagnostics and application behavior remain intact.

Fetch failures that prevent a usable response continue through Makepad's
existing error path. If the wrapper can observe the failure, it uses the same
error state; the loader must never manufacture a successful phase transition.

## Scope and ownership

The change stays in `scripts/inject-runtime-shell.mjs`, which already owns the
generated loader markup, CSS, fetch wrapper, and update-check runtime. The
checked-in `waml.svg`, Makepad fork, and Rust startup code do not change.

No granular Rust startup instrumentation is added. The exact compilation
completion and first-presented-frame seams provide enough honest feedback
without coupling the branded shell to application initialization internals.

## Verification

Automated coverage will exercise the injected artifact rather than the source
template alone:

- the generated SVG exposes six ordered progress segments and no clip rectangle;
- streamed byte progress advances segment levels sequentially and monotonically;
- resolving and rejecting the wrapped compilation promise selects the expected
  phase or error state without changing promise semantics;
- first loader removal produces exactly one fading overlay and final cleanup;
- reduced-motion CSS disables the chase;
- injection remains idempotent.

A browser smoke check will load a built artifact with throttling, confirm the
download sequence and phase labels, and confirm that the first app frame appears
through a crossfade without a blank or snapped frame.
