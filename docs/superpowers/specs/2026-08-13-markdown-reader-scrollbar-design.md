# Markdown Reader Scrollbar Design

## Goal

Make the vertical scrollbar visible in the Markdown reading view when the
document is taller than the viewport. Keep the current wheel, trackpad, touch,
selection, and source-handoff behavior unchanged.

## Design

The existing `viewer_body` remains the only scroll owner. Its `ScrollYView`
will use the same scrollbar presentation as WAML's Markdown editor and other
scrolling panels:

- a 5-pixel handle;
- `atlas.text_dim` at rest;
- `atlas.accent` on hover and drag.

Makepad keeps responsibility for overflow detection, handle sizing, position,
and visibility. The change adds no new widget, state, or event path. Documents
that fit in the viewport do not show an unnecessary scrollbar.

## Verification

Add a layout contract test that constructs the application UI and checks the
reading scroller's resolved vertical scrollbar style. Keep the existing mounted
wheel regression test. Verify the editor natively with the Mermaid reading
fixture and inspect a screenshot with overflowing content.

## Scope

Only the reading scroller style and its regression coverage are in scope. Do
not change the reusable Markdown viewer, Makepad, the Markdown editor scrollbar,
or scroll event routing.
