# Activity Diagram Icon Design

## Purpose

Replace the current heartbeat glyph for `Icon::Activity` with a glyph that
reads as a UML activity diagram. Keep the icon consistent with the Lucide
icons used by the editor.

## Selected Design

Use one large, centered outline diamond on Lucide's 24 by 24 grid. Use
Lucide's two-unit stroke, round caps, and round joins. Keep the diamond inside
the standard two-unit margin.

Do not add nodes, connectors, text, color, a container, internal marks, or
branches. The single geometric silhouette must remain clear at 12 to 16 px.

## Alternatives Considered

- A branching flow shows activity semantics, but it becomes crowded at 16 px.
- A start-action-decision-end stack has stronger UML detail, but its nodes and
  connectors become unclear at small sizes.
- A start-decision-end stack is simpler, but its small terminal nodes still
  create visual noise.
- The selected diamond is less literal, but it has the strongest silhouette
  and the most consistent small-size rendering.

## Integration

Replace `crates/waml-editor/resources/icons/activity.svg`, then use the
existing icon generator to update `IconActivity` in
`crates/waml-editor/src/icons.rs`. Keep the public `Icon::Activity` identity
and catalog mapping unchanged.

## Validation

- Compare the icon with adjacent Lucide glyphs in the icon harness.
- Check the icon at 12, 16, 20, and 24 px and at native HiDPI scale.
- Confirm that the generated SDF has no clipped geometry or uneven strokes.
- Run the icon catalog tests and the relevant editor checks.
