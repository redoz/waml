# FPS-heat top-bar wordmark

## What

The top-bar WAML wordmark (`LogoMark`, mode 0) tints toward a framerate heat colour
— **green (smooth) → amber → red (janky)** — while a pointer interaction is live
(node drag, pan, canvas animation). At true idle it is the normal grey wordmark;
on hover it keeps its existing shimmer, untouched. Playful, not a real FPS counter.

Only the top-bar instance (`auto == false`) meters. The splash logo (`auto == true`)
is unaffected — it keeps its colour-pulse variants.

## Why this scope (recap of decisions)

- **Honest meter, idle-free (option A2).** An FPS reading only means something while
  frames are actually being produced. makepad has no steady app-wide clock — frames
  are redraw-bursts driven by input. So the meter is gated to the span of a pointer
  interaction, where continuous frames genuinely happen and where real load lives
  (dragging a big graph). Idle → no frames scheduled by the meter → zero cost, grey.
- **Hover is sacred.** Hovering the wordmark runs its current traveling-wave shimmer
  exactly as today. Hover always wins over the meter (they rarely coincide anyway —
  during a drag the cursor is on the canvas, not the logo).

## Clean coupling (the seam)

Two components, one narrow interface. **No FPS logic in `App`; no canvas/interaction
logic in `LogoMark`.**

```
App  ──set_frame_metering(cx, on: bool)──▶  LogoMark
```

- **`App`** owns *interaction-span detection only*. It watches raw `Event::MouseDown`
  / `Event::MouseUp` (delivered to `AppMain::handle_event` regardless of which child
  hit-tests the press) and flips one boolean on the top-bar `LogoMark`:
  - `MouseDown` → `set_frame_metering(cx, true)`
  - `MouseUp`   → `set_frame_metering(cx, false)`
  That is App's *entire* involvement — it knows nothing about framerate, colour, or
  easing.
- **`LogoMark`** owns *all* metering: sampling frame dt, smoothing to FPS, mapping to
  a heat colour, easing in/out, and driving the shader. `App` never sees any of it.

`set_frame_metering` is a no-op when `self.auto` (splash never meters), so the call
site needs no branching.

## LogoMark internals

New `#[rust]` state:
- `metering: bool` — App's interaction-span flag.
- `meter: f32` — eased 0..1 heat strength (ramps up while metering & not hovered,
  down otherwise). This is what the shader reads, so enable/disable is smooth.
- `fps: f32` — EMA-smoothed framerate.
- `fps_color: [f32; 3]` — current heat colour (pushed as a uniform).

Frame loop (`handle_event`, `next_frame`, mode-0 branch only):
1. Target for `meter` = `if hovered { 0.0 } else if metering { 1.0 } else { 0.0 }`
   — **hover always suppresses heat.**
2. While `metering || meter > 0.0`, keep the loop armed (so it eases out cleanly),
   else stop (idle → zero cost, unchanged).
3. On each frame while metering: `dt = ne.time - last_time`, clamp to a sane range,
   `inst_fps = 1/dt`, EMA into `fps` (`alpha = 1 - exp(-dt / TAU)`, `TAU ≈ 0.15s`).
   **Skip the first sample after enable** (stale `last_time` → huge dt → false red
   flash): on enable set `last_time = now` and don't update `fps` that tick.
4. Map `fps → fps_color` (piecewise lerp, Rust):
   - `>= 60` → green `(0.235, 0.745, 0.353)`
   - `30..60` → amber→green
   - `15..30` → red→amber
   - `<= 15` → red `(0.922, 0.275, 0.471)`
   - amber `(0.902, 0.588, 0.078)`
5. Ease `meter` toward its target by `dt / METER_SECS` (`≈ 0.2s`).

`draw_walk` pushes two new uniforms: `fps_color` and `meter`.

## Shader (`mod.draw.LogoMark`)

Two new uniforms, independent of `mode`:
- `fps_color: uniform(vec3)` — heat target, default grey/unused.
- `meter: uniform(0.0)` — heat strength.

After the existing per-segment colours `kg1..kg6` are resolved (mode 0 / shimmer
path), apply one flat tint before compositing:

```
kgN = mix(kgN, self.fps_color, self.meter)
```

Because the meter only rises while `hover == 0`, the shimmer term is already at rest
when heat shows, so the two never fight. All six bars tint to the same heat colour —
the whole W flushes green→amber→red. Fold-order overlap keeps the shape legible.

Non-metered instances (splash, harness, start-screen card) leave `meter` at its `0.0`
default → wordmark unchanged.

## Testing

- `logo_harness` bin renders `LogoMark` standalone — add a keypress or timed sweep to
  drive `meter`/`fps_color` through green→amber→red for a visual check without needing
  a laggy graph.
- Manual: drag a large graph in the editor, confirm the wordmark heats up under load
  and eases back to grey on release; confirm hover still shimmers; confirm idle is
  grey and schedules no frames.

## Non-goals

- No numeric FPS text. No config toggle. No metering on the splash logo.
- Not a profiling tool — cosmetic, playful load indicator.
