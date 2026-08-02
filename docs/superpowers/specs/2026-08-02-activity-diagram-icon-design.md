# Activity Diagram Icon Design

## Purpose

Replace the current heartbeat glyph for `Icon::Activity` with a glyph that
reads as a UML activity diagram. Keep the icon consistent with the Lucide
icons used by the editor.

## Selected Design

Use a compact vertical activity flow on Lucide's 24 by 24 grid:

1. A small filled start circle at the top.
2. A rounded action node below it.
3. A decision diamond below the action.
4. A small end ring at the bottom.
5. A centered transition line that connects all four nodes.

Use Lucide's two-unit stroke, round caps, and round joins. Keep the outer
geometry inside the standard two-unit margin. Do not add text, color, a
container, or extra branches. The simple vertical silhouette must remain
clear at the editor's small icon sizes.

## Alternatives Considered

- A branching flow shows activity semantics, but it becomes crowded at 16 px.
- An action node with one arrow is clearer, but it looks like a generic flow
  or navigation command.
- The selected start-action-decision-end stack uses UML-specific shapes and
  has the strongest activity-diagram identity.

## Integration

Replace `crates/waml-editor/resources/icons/activity.svg`, then use the
existing icon generator to update `IconActivity` in
`crates/waml-editor/src/icons.rs`. Keep the public `Icon::Activity` identity
and catalog mapping unchanged.

## Validation

- Compare the icon with adjacent Lucide glyphs in the icon harness.
- Check the icon at its normal editor size and at native HiDPI scale.
- Confirm that the generated SDF has no clipped geometry or uneven strokes.
- Run the icon catalog tests and the relevant editor checks.
