# Veil cross-axis fade + un-armable conflict wedges

Two small, independent drag-to-place refinements to the constraint veil and the
drop dial. Both are native-only (the veil + dial live in `waml-editor`; the wasm
solver ABI is untouched).

## Problem

1. **The veil is a wall, not a cloud.** `ConstraintVeil` fades its hatch alpha
   only along the *extend* axis (the direction the placement points). On the
   perpendicular (unlocked) axis the band spans the whole viewport at full
   strength with a hard edge. A `LeftOf` veil is a horizontal strip cutting the
   canvas top to bottom — it reads as a barrier rather than a region hugging the
   reference.

2. **A drop can author a conflicted layout.** The dial's wedges already
   speculative-solve and mark conflicting directions with the `danger` (red)
   flag, but a red wedge is still fully armable and committable. Releasing on one
   pushes `Op::PlaceSet` and drops the diagram into a conflicted state. The user
   should not be able to place the diagram into conflict at all.

## Feature 1 — cross-axis veil fade

Add a symmetric fade on the veil's *cross* (unlocked) axis, centred on the
reference node's perpendicular span and decaying to zero toward the band's long
edges. Combined with bounding the drawn band on that axis to a reach around the
reference (it currently spans the full view), the veil reads as a soft blob
hugging the reference instead of an infinite strip.

**Semantics are unchanged.** The hatch is a visual affordance only. Which cards
desaturate is decided by the world-space `keep_out` half-plane in `veil.rs`
(`desaturated_cards`), which is independent of the drawn hatch rect. Bounding and
fading the hatch on the cross axis changes only what is painted, not what the
placement forbids.

### Touch points

- `veil_band` (`canvas.rs`): on the unlocked axis, replace the full-view extent
  with a bounded band centred on the reference's cross span (reference extent
  grown by the reach on each side), instead of `view.pos`/`view.size`.
- `veil_ramp` / a new companion (`canvas.rs`): produce the cross-fade parameters
  — the normalized centre and half-span of the plateau→0 falloff on the cross
  axis — alongside the existing extend `ramp`/`bias`. The locked (extend) axis
  gets no cross fade; the unlocked axis carries the real values.
- `ConstraintVeil.pixel` (`canvas.rs` `script_mod`): compute a symmetric cross
  fade `cross = 1 - clamp((|pos.axis - centre| - plateau) / soft, 0, 1)` on the
  unlocked axis and multiply it into the existing `fade`. The extend-axis anchor
  edge stays crisp — this touches the cross axis only.

### Testing

`veil_band`'s new bounded cross extent is pure and unit-tested like the existing
`veil_band_anchors_and_clamps_per_direction`: assert the unlocked axis is now a
reference-centred band of the expected extent rather than the full view, per
direction. The shader `pixel` fn cannot run headless, so its cross-fade math is
locked by a code comment in the same idiom as the existing `ConstraintVeil` /
`GroupDashed` shader notes.

## Feature 2 — un-armable conflict wedges

Make conflicting dial wedges inert dead-zones: no hover-preview, no commit. The
entire disabled-wedge machinery already exists — this feature just wires two
switches to it.

### Existing machinery (no change)

- `marking.rs` `release`: only an `enabled` slot commits; a marking-drag release
  over a disabled slot falls through to `Cancelled`. So releasing on a disabled
  wedge dismisses the whole dial and the node eases home via the existing
  dismiss path — nothing authored.
- `resolve_in` (`marking.rs`): documents a disabled wedge as a dead-zone,
  equivalent to the hub.
- The wedge shader forces the flat-grey disabled look when `enabled = 0`
  (`state = 0`, dim-grey icon), which overrides `danger`. Conflicting wedges
  therefore render grey-dead.

### Changes

- **`class_diagram_view.rs`** (the `CompassArmed` dial-build): ship conflicting
  wedges disabled — `enabled: !red.contains(&z)`. Since the disabled grey look
  overrides `danger`, the now-redundant `danger: red.contains(&z)` is dropped to
  `danger: false` (grey-dead is the chosen look, KISS — no red-and-dead shader
  path).
- **`marking.rs`** (`pointer_move`): arm only enabled wedges —
  `self.armed = hit.filter(|&i| self.items[i].enabled)`. This kills the
  hover-preview on a disabled wedge (its `Armed` never fires, so no candidate
  layout previews). Universally correct: burger and logo menu items are all
  `enabled: true`, so no other surface changes behaviour.

### Result

A red/conflict direction is a grey dead-zone: it does not preview on hover and
does not commit on release. Releasing over one cancels the dial and the node
returns home. No conflicted layout is reachable through the drop dial.

### Testing

- `marking.rs`: a `pointer_move` onto a disabled slot leaves `armed()` `None`
  (add to the existing marking tests); the existing `release`-over-disabled
  cancel behaviour already has coverage.
- `class_diagram_view.rs`: extend the dial-build coverage so a direction the
  speculative solve reddens ships a wedge with `enabled == false`.
- Interactive: per-pid native sign-off — drag a node so at least one direction
  conflicts, confirm that wedge is grey + gives no preview, and that releasing on
  it authors nothing and eases the node home.

## Out of scope

- Red-and-dead wedges (danger hue retained while inert) — deferred; would need a
  shader path letting `danger` show through `enabled = 0`.
- Any change to the wasm solver ABI or the off-canvas conflict error list.
- The extend-axis near edge (stays crisp against the reference).
