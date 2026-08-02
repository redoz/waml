# Animated Dock Icons Design

## Goal

Keep the current left project-tree and right inspector behavior, but animate each panel when it opens or closes. Replace the static panel toggle glyphs with Lucide open and close glyphs that show the next available action.

## Scope

This change applies to the existing project-tree and inspector docks in `waml-editor`.

It adds these Lucide glyphs to the shared icon system:

- `folder-tree`
- `panel-left-open`
- `panel-left-close`
- `panel-right-open`
- `panel-right-close`

`folder-tree` becomes available in the shared icon catalog and icon harness. The dock controls use the four panel glyphs.

The change does not alter panel contents, panel widths, persistence, keyboard commands, responsive breakpoints, or narrow-mode mutual exclusion.

## Toggle Glyph Rules

Each toggle shows the action that a click will perform:

| Panel | Collapsed target state | Expanded target state |
| --- | --- | --- |
| Left project tree | `panel-left-open` | `panel-left-close` |
| Right inspector | `panel-right-open` | `panel-right-close` |

The glyph changes as soon as the logical target state changes. During a reversed animation, it therefore always shows the action for the current target state.

The shared icon enum, draw fields, glyph selection, labels, `Icon::ALL`, overlay groups, and icon-harness coverage must list the new glyphs in one consistent field order.

## Motion Model

The logical dock state stays binary: `Flag` or `Pinned`. Animation is a separate presentation value in the range `0.0..=1.0`:

- `0.0` means fully collapsed.
- `1.0` means fully expanded.
- Values between the endpoints define the visible body width and the reserved desktop slot width.

Both panels use a 180 ms ease-out transition. A click during a transition samples the current value and starts a new transition toward the opposite endpoint. This prevents jumps when the user reverses direction.

The app owns the two motion values because `App::sync_dock_slots` already owns left-slot, right-slot, tree-body, and inspector-body sizing. A small pure dock-motion type provides endpoint selection, sampling, reversal, and completion. A `NextFrame` request advances active motion.

## Layout Behavior

Wide mode keeps the present dock behavior:

- The animated body width is also the animated slot width.
- The center content resizes continuously as the panel opens or closes.
- The final left and right widths stay unchanged.

Narrow mode keeps the present overlay behavior:

- The center slots remain zero-width.
- The panel body width animates between zero and its capped target width.
- Opening one panel still closes the other panel.
- Existing outside-click dismissal stays unchanged.

Panel contents remain drawable until a closing transition reaches zero. This prevents an empty column from shrinking after the content has already disappeared. At zero width, the existing collapsed drawing path takes control again.

Window resize and responsive-mode changes retarget the active motion with the current sampled value. Widths stay clamped to the available viewport in narrow mode.

## Integration

`App::sync_dock_slots` continues to read the logical panel states. It maps those states to motion targets, samples each motion, and writes the resulting widths to `left_slot`, `right_slot`, `tree_host`, and `inspector_host`.

The tree caption button and document-header right-dock button receive their glyphs from the logical target state. Their active styling continues to follow the expanded target state.

The project-tree and inspector widgets receive a presentation-visible flag so they can draw throughout a closing transition without changing their logical dock state.

## Tests

Tests are added before production changes.

Pure tests cover:

- exact zero and one endpoints;
- monotonic eased interpolation;
- completion at 180 ms;
- reversal from the sampled in-flight value;
- stable behavior for repeated requests to the same target;
- wide-mode slot and body interpolation;
- narrow-mode zero slots and animated body widths;
- viewport clamping in narrow mode;
- left and right glyph selection for both logical states;
- all five icon labels and `Icon::ALL` ordering.

Existing dock, responsive-layout, document-header, and icon-harness tests must remain green. The editor is also built and the running window is checked to confirm smooth motion, correct glyph changes, and correct behavior when a transition is reversed.

## Acceptance Criteria

- Both panels keep their current final layout and responsive behavior.
- Opening and closing use a smooth 180 ms width animation.
- The center area resizes smoothly in wide mode.
- Panel content stays visible for the full close animation.
- A second click reverses an active animation without a width jump.
- Each toggle shows the correct open or close Lucide glyph.
- `folder-tree` is available through the shared icon catalog.
- Existing unrelated working-tree changes are not modified or committed.
