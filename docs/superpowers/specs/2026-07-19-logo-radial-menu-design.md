# Logo radial menu + shimmer hover — design

**Date:** 2026-07-19
**Target:** `waml-editor` (Makepad), immediate-mode
**Builds on:** `radial.rs` (radial command menu), `logo.rs` (SDF wordmark), `waml_button.rs` (hover/animation widget pattern)

## Summary

Make the top-bar WAML wordmark interactive. Left-clicking it opens the existing
radial menu in persistent popup mode with four slots — Properties (top), About
(right), Cancel (bottom), Exit (left). Hovering the wordmark plays a
shader-driven "shimmer": the accent colour flows across the six fold-segments as
a phase-offset traveling-wave-plus-pulse, easing in on enter and out on leave.

## Motivation

The wordmark is currently a passive `SolidView`. A logo click is a conventional
place for an app/global menu; the editor already has a polished radial widget
(built for node right-click), so we reuse it rather than inventing a second menu
idiom. The hover shimmer gives the mark life and doubles as the affordance hint
that it is clickable.

## Constraints discovered

- **Caption-bar drag region.** The wordmark lives inside the OS window-drag
  region of the caption bar. That region swallows both hover *and* click before
  they reach any widget (same problem `doc_tabs` has). `App` must answer
  `WindowDragQueryResponse::Client` over the logo's drawn rect — the exact trick
  at `app.rs:1038-1048` for the tab strip.
- **Radial opens on the secondary button.** `Radial::open` (`radial.rs:388`)
  begins in marking mode (`pressed = true`), driven by right-press gestures. A
  left-click needs to open straight into persistent popup mode instead.
- **Icons are shader SDFs**, not the `resources/icons` svgs. Wedge glyphs are
  `IconShape` enum branches drawn in `icon.rs`'s `pixel()` (`icon.rs:88-112`).
- **Primitives** (verified in the pinned makepad fork, rev `4f9ce7a`):
  `cx.open_url(url, OpenUrlInPlace::No)` and `cx.quit()`.

## Units

Four units. They can land in one branch; ordered so each compiles green.

### 1. `LogoMark` becomes a Widget (`logo.rs`)

Promote the bare `mod.draw.LogoMark` shader into a proper immediate-mode Widget
struct, mirroring `WamlButton`:

```rust
#[derive(Script, ScriptHook, Widget)]
pub struct LogoMark {
    #[uid] uid, #[source] source, #[walk] walk, #[layout] layout,
    #[redraw] #[live] draw_bg: DrawColor,   // the SDF wordmark shader
    #[rust] hovered: bool,
    #[rust] hover: f32,       // eased 0..1
    #[rust] anim_start: f64,  // wall-clock origin for `time`
    #[rust] next_frame: NextFrame,
    #[rust] area: Area,       // drawn rect, for the drag-query override
}
```

- `handle_event`: on `Hit::FingerHoverIn/Out` set `hovered` + kick the
  `NextFrame` loop; on `Hit::FingerDown` emit a `LogoClicked` action carrying the
  logo rect's centre. Uses `cx.hit(area, ...)` like a normal widget — this only
  fires once the drag-query override (Unit 3) lets events through.
- Animation loop: each `NextFrame`, ease `hover` toward `hovered ? 1 : 0`
  (~150 ms), push `hover` + `time = now - anim_start` as uniforms, redraw.
  When `hover` reaches 0 and `!hovered`, stop scheduling frames (idle = zero
  cost).
- `LogoClicked` is a `WidgetAction`-style action `App` reads from `actions`.

**Shader extension** (same `pixel`, added uniforms `hover`, `time`, `accent`):
per fold-segment `i` (six segments, left→right by x-centre approximately
0.15/0.31/0.47/0.62/0.75/0.88), compute
`glow_i = hover * clamp(pulse_i + wave_i, 0, 1)` where

- `pulse_i = 0.5 + 0.5*sin(time*W - i*PHI)` — phase-offset breathe/shimmer
- `wave_i  = exp(-((xc_i - fract(time*SPEED))^2) / WIDTH)` — a bright band
  traveling left→right and looping

Mix each segment's themed grey toward `accent` by `glow_i` **before** the
existing fold-order "over" composite (so the accent inherits the ribbon's clean
seams). Constants `W/PHI/SPEED/WIDTH` are screenshot-tuned in the harness.

### 2. Radial popup-open (`radial.rs`)

Add a click-open entry that skips marking mode:

```rust
impl RadialCore { pub fn begin_popup(&mut self, center, items) { /* open=true, popup=true, pressed=false, dragged=false */ } }
impl Radial     { pub fn open_popup(&mut self, cx, center, items, time) { ... } }
```

The existing `handle()` already routes a primary `MouseDown` -> `click()` ->
commit, and hub / outside-disc / `Esc` -> cancel, so no other change is needed.
Covered by a unit test alongside the existing `RadialCore` tests (tap-opens-popup
already exists; add begin_popup-opens-popup + primary-click-commits).

### 3. `App` wiring (`app.rs`)

- **Wordmark swap:** replace the `SolidView{ draw_bg: mod.draw.LogoMark{} }` at
  `app.rs:58-62` with the new `LogoMark{}` widget (id `logo`).
- **Drag override:** in the `Event::WindowDragQuery` block (`app.rs:1038`), also
  set `Client` when `dq.abs` is inside the logo widget's `area` rect — otherwise
  hover/click never arrive.
- **Open on click:** when the `LogoClicked{center}` action appears in `actions`,
  call `radial.open_popup(cx, center, logo_radial_items(), cx.seconds_since_app_start())`.
- **Command mapping:** add `logo_radial_items()` (parallel to
  `node_radial_items()`, `app.rs:604`) and a `logo_command_for(id)` mapper. In
  the radial-outcome handler (`app.rs:1017`), try `logo_command_for` in addition
  to `node_command_for`:

```rust
enum LogoCommand { Properties, About, Exit }   // Cancel handled by radial close
fn logo_command_for(id) -> Option<LogoCommand> { properties|about|exit -> ... }
```

  - Properties -> `log!` stub (no-op for now).
  - About -> `cx.open_url("https://github.com/redoz/waml", OpenUrlInPlace::No)`.
  - Exit -> `cx.quit()`.
  - Cancel wedge -> id `cancel`, maps to nothing; the radial closes on commit.

### 4. Wedge icons (`icon.rs`)

Add four `IconShape` branches (shader SDFs, screenshot-tuned), one per slot:

| slot | item | IconShape | danger |
|------|------|-----------|--------|
| top (0) | Properties | `Properties` (gear/sliders) | no |
| right (1) | About | `About` (info "i") | no |
| bottom (2) | Cancel | `Cancel` (X) | no |
| left (3) | Exit | `Exit` (power/logout) | yes (red tint) |

Reusing `IconShape::Remove` (an X) for Cancel is acceptable if it reads cleanly;
final glyph pick and tuning happen in the harness. Each new variant extends the
`shader_index()` mapping and the `pixel()` if/else chain — same additive pattern
the existing four use.

## Data flow

```
FingerDown on logo
  -> LogoMark emits LogoClicked{center}
  -> App: radial.open_popup(center, logo_radial_items())   [popup mode]
  -> user primary-clicks a wedge
  -> Radial::handle -> RadialOutcome::Committed(id)
  -> App: logo_command_for(id) -> { Properties: log | About: open_url | Exit: quit }
       (Cancel / hub / Esc -> RadialOutcome::Cancelled -> nothing)
```

Hover is independent of the menu: it runs purely inside `LogoMark`'s NextFrame
loop while the pointer is over the mark, driving shader uniforms.

## Error / edge handling

- **Menu open while already open:** a second logo click re-opens with a fresh
  centre (harmless; `begin_popup` resets state).
- **Exit = immediate `cx.quit()`.** No unsaved-work guard — there is no
  document-dirty state in the editor yet. (Revisit when persistence lands.)
- **`open_url` failure** is the OS's problem; nothing to recover in-app.
- **Hover during menu:** shimmer keeps running under the open disc; acceptable.

## Testing

- `radial.rs` unit tests: `begin_popup` opens in popup mode; a primary click on
  an enabled wedge commits; hub/outside/Esc cancel. (Extends existing pure-core
  tests — no GPU.)
- `logo_command_for` mapping test (ids -> commands, unknown -> `None`), mirroring
  the `node_command_for` test at `canvas.rs:630`.
- Hover shimmer + wedge icons are visual — tuned and verified in `logo_harness`
  via self-screenshot (the established recipe), not asserted in unit tests.

## Out of scope

- Properties actually doing anything (stubbed).
- Unsaved-work / quit confirmation.
- Keyboard-opening the logo menu; touch/marking-drag from the logo.
- Any change to the node right-click radial behaviour.

## Files touched

- `crates/waml-editor/src/logo.rs` — Widget struct + hover animation + shader shimmer
- `crates/waml-editor/src/radial.rs` — `begin_popup` / `open_popup`
- `crates/waml-editor/src/app.rs` — wordmark swap, drag override, click->open, `logo_radial_items`/`logo_command_for`
- `crates/waml-editor/src/icon.rs` — four wedge `IconShape` variants
- `crates/waml-editor/src/bin/logo_harness.rs` — hover-shimmer preview for tuning
