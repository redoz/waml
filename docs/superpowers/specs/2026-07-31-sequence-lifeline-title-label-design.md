# Sequence Lifeline Title Label

## Problem

A resolved sequence lifeline currently displays its authored title and its
canonical reference key as one label:

```text
Author:architecture/concepts/workflows/author
```

The canonical key makes the lifeline head much wider than its useful display
text. It also exposes an internal lookup value in the diagram.

## Design

Display only the authored lifeline title in the lifeline head. Continue to
store and use the resolved reference key for navigation, selection, and accent
styling.

Use the same title-only label when the interaction solver measures the
lifeline head. This keeps the measured width equal to the rendered width.

Reference resolution and diagnostics do not change. An unresolved reference
continues to produce an `UnresolvedTarget` diagnostic.

## Tests

- A resolved lifeline scene label contains only the authored title.
- A resolved lifeline head is measured from the authored title, not from the
  canonical reference key.
- Existing resolution, navigation, and interaction tests continue to pass.

## Scope

This change affects only sequence lifeline label display and measurement. It
does not change source syntax, model serialization, reference resolution, or
other diagram types.
