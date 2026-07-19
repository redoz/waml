# Radial command menu — design

**Date:** 2026-07-19
**Target:** `waml-editor` (Makepad), immediate-mode
**Mock reference:** `docs/design/hud-tag-radial-mock.html` (tag-scoped precursor; this generalises it)

## Summary

A dynamic radial (marking) menu that shows 2–6 commands as pie-sector wedges
around a central hub. Opened on right-click of a canvas node; the same widget
is reusable for any target by handing it a fresh command list per open. It
supports two ways in on the **right mouse button**: a quick right-click opens a
persistent popup you then click; a right-click-hold-and-drag is a marking-menu
gesture that commits on release. Wedge glyphs come from a new generic `icon`
abstraction (shader-drawn SDF, not a font atlas), which ships alongside the
radial and is reusable by the rest of the editor later.

## Motivation

For a small set of commands (≤6), a radial reads faster and hits faster than a
linear popup menu, and it supports expert muscle-memory via marking gestures.
The existing `hud-tag-radial-mock.html` proved the look but is hard-wired to
four tag actions. We want one dynamic widget the whole editor can reuse.

## Units

Three units land together.

### 1. `icon` module (`crates/waml-editor/src/icon.rs`)

A generic icon abstraction, so wedges (and later `tool_dock`, `tree`, `card`)
reference icons without knowing how they paint. Not a font atlas — icons are
shader-drawn SDFs (matching how the mock hand-draws its glyphs).

```rust
enum Icon {
    Glyph(char),      // DrawText — the current house placeholder, still valid
    Shape(IconShape), // shader-drawn SDF
    // Texture(TextureId) later — additive, no API break
}

enum IconShape { Open, Style, Markdown, Remove } // grows one branch at a time

fn draw_icon(cx: &mut Cx2d, draw: &mut DrawIcon, rect: Rect, icon: &Icon, tint: Vec4);
```

- `DrawIcon` is one `DrawColor`; its `pixel()` switches on a `shape` uniform
  index, each branch drawing that icon's SDF (stroke/fill ops). `tint` colours
  the icon (accent for normal wedges, danger for `Remove`, dim for disabled).
- Seed set for v1 is exactly the four node-radial commands. Adding an icon =
  one `IconShape` variant + one `pixel()` branch.
- `Icon::Glyph(char)` keeps the existing single-char `DrawText` path valid, so
  callers that don't yet have an SDF shape can still show something.

### 2. `radial` module (`crates/waml-editor/src/radial.rs`)

The dynamic 2–6 wedge menu. Immediate-mode component, same convention as
`button`/`tool_dock`: the parent owns placement + drives it through inherent
methods; it does not self-route tree events.

```rust
struct RadialItem {
    id: LiveId,        // reported back on commit; radial owns no command semantics
    label: String,
    icon: Icon,
    danger: bool,      // danger-token hue across all wedge states
    enabled: bool,     // false = greyed, holds its slot, cannot arm/commit
}

enum RadialOutcome { Committed(LiveId), Cancelled, None }

impl Radial {
    fn open(&mut self, cx: &mut Cx, center: DVec2, items: Vec<RadialItem>, time: f64);
    fn handle(&mut self, cx: &mut Cx, event: &Event) -> RadialOutcome;
    fn draw(&mut self, cx: &mut Cx2d); // draws at the stored center
    fn is_open(&self) -> bool;
}
```

### 3. Node radial wiring (canvas)

Canvas right-click / right-press on a node opens the radial at the cursor with
`[Open, Style, Markdown, Remove]` (Remove = danger). The committed `LiveId`
routes into the existing node command path. `Open`/`Style`/`Markdown`/`Remove`
map to their handlers; `Cancelled`/`None` do nothing.

## Geometry

Layout A — equal full-circle sectors.

- N items → N sectors of `360/N°`. First wedge **centred at 12 o'clock**,
  proceeding clockwise. Wedge directions therefore shift with N (accepted
  trade-off; stable compass slots were explicitly not wanted).
- Fixed disc radius (~120px screen-space), central **hub dead-zone** radius
  ~30px = cancel target and neutral origin.
- Each wedge = pie sector: two accent-hairline spokes + a per-slice rim arc, so
  a wedge's outline fades together with its fill (mock behaviour).
- Icon + label centred on the sector mid-angle at a fixed radius between hub and
  rim. Icon upright (never rotated); label in sans beneath it.

### Hit-test — angle from centre (both modes)

- Cursor inside the hub dead-zone → nothing armed (cancel zone).
- Outside the hub → wedge index = `floor(((angle - start_angle) mod 360) / sector)`.
- Because hit-testing is by angle, screen-edge clipping of the drawn disc
  (see Placement) never affects which wedge is pickable.
- A wedge with `enabled == false` is treated like the dead-zone: angle over it
  arms nothing; a commit attempt on it is a no-op and leaves the radial open.

## Interaction

Both entries are on the **right mouse button**; the state machine chooses popup
vs marking by whether the pointer held-and-dragged past a short threshold before
release.

- **Right-click (tap — press then release without meaningful drag) → persistent
  popup.** The radial stays open. Moving the pointer highlights the hovered
  wedge (`hover` state). A subsequent click on a wedge → `Committed(id)`.
  Clicking the hub, clicking outside the disc, or `Esc` → `Cancelled`.
- **Right-click + hold + drag → marking menu.** While the button is held, the
  wedge under the current angle *arms*; the others recede in proportion to how
  far the cursor rides toward the rim (`dim` mid-swipe, `gone` near the rim) —
  fill, glyph, and rim-arc fading together. Releasing the right button over an
  armed wedge → `Committed`. Releasing inside the hub dead-zone → `Cancelled`.
  A flick past the rim commits (`is-flick`).

There is no left-click / "quick press" entry.

### Dismiss paths (all four)

1. Click the hub.
2. Click outside the disc.
3. `Esc`.
4. Marking gesture: release inside the hub dead-zone.

Outcome to the caller is always one of `Committed(LiveId)` / `Cancelled` /
`None`; the radial owns none of the command semantics — the parent maps the id.

## Rendering & material

Reuses the accent-frame look (shared stroke recipe with `AccentFrame`; see
Rename note).

- **Disc:** frosted white fill (`rgba(255,255,255,.90)`), depth drop-shadow plus
  a low accent bloom (mock `.dial` filter).
- **Wedge fill by state:** accent `.05` rest → `.15` hover → `.18` arming →
  `.28` flick (+ emissive edge glow). A `danger` wedge swaps accent→danger token
  across the same ramp.
- **Spokes + per-slice rim arcs:** source-bright accent stroke using the same
  150° fade material as `AccentFrame`, factored so the radial and the frame
  share one stroke recipe.
- **Hub:** white fill, accent ring, grey ✕.
- **Disabled wedge:** flat grey fill, dimmed label/icon, no hover/arm response.

**Shader shape:** one `DrawColor` per wedge, drawn with `draw_abs` (N wedges per
frame). `pixel()` renders the SDF pie-sector + arc + spokes. Per-wedge uniforms:
`state` (rest/hover/arm/flick), `danger`, `enabled`. Icons drawn over each wedge
via `draw_icon`; labels via `DrawText`.

**Animation** (a `NextFrame` loop, same pattern as `WamlButton::tick`):

- **Bloom-in** on open (~120 ms scale + opacity).
- **Popup:** hovered wedge takes the `hover` fill (near-instant).
- **Marking:** per frame, compute cursor radius; passed-over wedges take
  `dim`/`gone` by distance-to-rim; the armed wedge brightens; a flick past the
  rim flares (`is-flick`).

## Placement

Disc centre sits at the press / right-click point (= the marking origin). When
that point is near a screen edge and the disc would clip, the disc is drawn
**clipped** (not nudged) — muscle-memory direction to each wedge stays constant,
and angle-based hit-testing keeps every wedge pickable regardless.

## Accent

Uses the theme accent token; `Remove` (and any `danger` wedge) uses the danger
token. No per-open accent recolour (that was mock-only tooling).

## Out of scope (YAGNI)

- Nested / sub-radials.
- More than 6 items.
- Per-open accent recolouring.
- Web / Svelte frontend (this is Makepad-only for now).

## `AccentFrame` rename (separate, non-blocking)

Independently of this feature, the mock-slang "HUD" names get corrected:

| Now | New |
| --- | --- |
| `draw_hud.rs` / `HudFrame` | `frame.rs` / `AccentFrame` |
| `waml_button.rs` / `WamlButton` | `button.rs` / `Button` |
| DSL `mod.draw.HudFrame` | `mod.draw.AccentFrame` |
| DSL `mod.widgets.WamlButton` | `mod.widgets.Button` |

`AccentFrame` (not `AtlasFrame` / `Frame`): describes the visual, and avoids
every widget carrying the theme name once a second theme exists. This rename is
tracked as its own small change; the radial reuses the frame *material* either
way and does not block on it.

## Testing

Matches repo convention — `cargo test` unit tests plus a manual self-screenshot
(the Makepad self-screenshot recipe).

- **Geometry / hit-test units** (no GPU): `angle → wedge index` for N = 2..6;
  hub dead-zone returns `None`; disabled wedge returns `None`; wrap-around at
  12 o'clock.
- **State-machine units:** tap → popup → commit; hold-drag → arm → release-commit;
  each cancel path; flick-commit.
- **Manual screenshots:** each N (2..6) plus hover / arm / flick / disabled
  states for visual sign-off.
