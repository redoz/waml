# Responsive viewport chrome

Status: designed, not implemented.

## Goal

`waml-editor` has one fixed desktop composition. Its 70x40 wordmark occupies a
full-height column beside both caption rows, document tabs fade beyond the
available strip instead of remaining reachable, and pinned docks reserve their
full widths regardless of the viewport. At a phone-width viewport (~390px),
the caption is cramped and the 280px tree plus 320px inspector can consume more
than the entire canvas.

Introduce two width-driven chrome modes:

- **Wide** keeps the multi-document tab strip and reserved dock columns.
- **Narrow** replaces the strip with an active-document chip and draws docks
  over the full-width canvas.

Both modes use the same compact logo in the title row and the same two-row,
full-width caption. Resizing changes presentation only: it never changes the
set of open documents or loses panel state.

## Decisions

- **Two modes only.** Narrow and wide share one breakpoint state. Separate
  caption and dock breakpoints were rejected because their combinations would
  create implicit third and fourth modes.
- **Width threshold, not platform.** A desktop window dragged narrow must show
  the same layout as a phone-width web viewport so the design is observable and
  testable without device gating.
- **One logo instance.** The old full-height wordmark column is removed. A
  single 44x25 `LogoMark` lives in `title_row` in both modes, avoiding duplicate
  menu, heat-meter, and drag-query wiring.
- **Caption and docks change together.** The breakpoint is reconciled on the
  path that already runs `sync_dock_slots`, giving the entire chrome one
  relayout trigger.
- **Tabs collapse to a chip plus switcher.** Narrow mode does not close or cap
  open documents. The active document remains directly closable and all open
  documents remain reachable through a popup.
- **Narrow docks overlay and are mutually exclusive.** The canvas retains the
  full viewport width. Wide mode continues to allow both reserved docks.
- **Existing `DockState` remains authoritative.** Responsive layout does not
  add parallel tree-open or inspector-open booleans.

## Design

### Breakpoint

`App` gains a `narrow: bool`, recomputed from the viewport width during
`sync_dock_slots`.

- Enter narrow below 640px.
- Leave narrow above 680px.
- Preserve the current mode from 640px through 680px.

The 40px hysteresis prevents a window resting near the boundary from reforking
the full chrome every frame. One shared mode is deliberately conservative:
although moving the logo frees tab width, reserved docks still leave too little
canvas below the threshold.

When the mode changes, `App` requests one full relayout and dismisses a
document switcher opened under the previous composition because its anchor is
no longer valid.

### Caption hierarchy

The 66px caption remains two rows, but the entire caption becomes one
full-width column:

```text
title_row visual: logo(44) | menu_btn(30) | model_name(Fill) | window_buttons
tab_row:   tree_btn(30) | tree_gap | doc_tabs(Fill) | inspector_btn(30)
```

The old direct-child `wordmark` view is removed. `caption_col` becomes the
caption bar's full-width child. `agent_mark` remains the first DSL child at
zero width so its absolute wash paints underneath the row; `logo` follows as
the first visible-width child. Its 44x25 box preserves the mark's documented
~1.749 aspect ratio.

The existing `LogoMark` behavior stays single-sourced:

- `WindowDragQuery` client-izes its one drawn rect.
- `logo_action` opens the app menu.
- the FPS heat meter updates the same widget.

Because the logo now sits in the upper row, its menu anchors below the logo
like the burger menu instead of assuming a full-band logo and clamping from
that geometry. The caption DSL comments must describe the actual nesting and
client-area rule; the obsolete claim that the logo and burger must remain
direct caption children is removed.

`tree_btn` is the first child of `tab_row`, so it floats at the viewport's left
edge underneath the logo rather than inheriting a full-height logo offset.
`inspector_btn` remains the last child and stays flush right.

### Wide tab row

Wide mode retains the normal multi-document `DocTabs` rendering.

`tree_gap` remains derived from the reserved left slot:

```text
max(0, left_slot_width - TREE_BTN_W)
```

This keeps `[T]` fixed at x=0 while aligning the first tab card with the
canvas's left edge when the tree is open. When the tree is closed, the gap is
zero and the existing `TAB_LEFT_INSET` separates the first card from `[T]`.

The top rule now extends left from the strip to x=0 because no wordmark occupies
the lower row. Its right endpoint continues to use the current conditional
inspector-button overshoot. The earlier concern about an unconditional
`WINDOW_BUTTONS_W` overshoot is obsolete in the current code and is outside
this work.

### Narrow document chip and switcher

In narrow mode, `DocTabs` draws only the active document as one chip. It fills
the available strip up to a 320px cap and keeps the active tab's icon, title,
preview styling, and close button. Narrow title truncation may use a larger
character limit because one card owns the strip.

- Pressing the chip body emits a new switcher request carrying its
  bottom-left anchor.
- Pressing `x` emits the existing `DocTabsAction::Close(active_id)`.
- With no active document, no chip is drawn and no switcher request is emitted.

`App` handles the request by constructing one existing `PopupItem` per open
document and opening the shared `PopupRoot` under a dedicated
`doc_switcher` tag. Each row contains the document icon and title; the active
row is marked through `open_marking`. Existing `MenuPopup` maximum-height,
scroll, and thumb behavior handles long lists. A committed row activates the
selected tab through the same refresh and active-view synchronization path as
a wide tab click.

No secondary close action is added to `PopupItem`; closing remains on the chip.

### Dock state and layout

`ProjectTree::dock_state()` and `Inspector::dock_state()` are the open/closed
source of truth. `DockState::Pinned` means open and `DockState::Flag` means
closed for caption-toggle behavior.

In wide mode:

- `left_slot` reserves `ProjectTree::slot_width()`.
- `right_slot` reserves `Inspector::slot_width()`.
- both panels may be open.

In narrow mode:

- both reservation slots are `Size::Fixed(0.0)`;
- a pinned tree draws absolutely from the left edge at its normal 280px width;
- a pinned inspector draws absolutely from the right edge at its normal 320px
  width;
- panel width is capped to the available viewport;
- each panel spans from caption bottom to statusbar top.

The existing panel state survives a mode change. Narrowing while both wide
docks are open keeps the tree and closes the inspector, making mutual
exclusion deterministic. Widening an open narrow panel turns it back into a
reserved column without changing its state.

`[T]` and `[I]` derive their lit state directly from `DockState`, while wide
slot widths and narrow overlay visibility are separately derived from that same
state. Slot width is no longer used as a proxy for whether an overlay is open.

### Narrow dock interactions

Opening `[T]` in narrow mode closes the inspector before toggling the tree.
Opening `[I]` closes the tree before toggling the inspector. A view-side request
to open the inspector follows the same mutual-exclusion rule.

A primary press on the canvas outside the open overlay closes it. Whether the
pointer lies inside the panel is maintained from raw mouse movement, not
`Hit::FingerHover`, because panel children may claim hover before the panel.
The overlay must consume hits within its own bounds so canvas gestures do not
fall through.

Wide mode retains today's independent dock toggles and permits both panels to
remain open.

## Edge cases

- The start screen keeps the logo available; editor-only caption controls
  remain hidden through their existing visibility paths.
- No active document produces no narrow chip or empty switcher.
- A mode transition dismisses an open document switcher.
- A viewport narrower than a dock clamps the overlay to the viewport width.
- A right dock that is unavailable for the active view remains closed and its
  toggle remains hidden through `sync_right_dock_btn`.
- Resizing never opens, closes, promotes, or removes documents.

## Non-goals

- Statusbar, tool dock, and unrelated popup sizing are not audited.
- No touch-specific gestures are introduced.
- Wide-mode horizontal tab scrolling is not built; it keeps the existing fade.
- Document opening, closing, preview promotion, and persistence semantics do
  not change.
- The desktop window-button implementation is not changed.

## Risks

- **Absolute dock hit routing.** A panel drawn over the canvas must consume hits
  inside its bounds without competing with `PopupRoot`. If partial-width
  overlays prove incompatible with the overlay authority, the fallback is a
  full-viewport drawer.
- **Breakpoint transition with two open docks.** Wide mode permits a state
  narrow mode cannot display. The tree-wins rule must execute once on entry,
  not every frame.
- **Nested caption controls.** The logo and burger now live inside
  `title_row`. Every interactive caption child must remain client-ized in
  `WindowDragQuery`.
- **Hysteresis masking transition defects.** Interactive verification must drag
  through both boundaries, not only sample one width on each side.

## Verification

Unit:

- breakpoint transitions enter below 640px, leave above 680px, and preserve
  state throughout the hysteresis band;
- wide and narrow reservation widths derive correctly from the same
  `DockState`;
- entering narrow with both docks open keeps the tree and closes the inspector;
- opening either narrow dock closes the other;
- switcher item construction preserves open-document order and marks the active
  row;
- a narrow chip requests the switcher from its body and closes only from `x`;
- the top rule reaches x=0 in both modes and keeps the conditional right edge.

Interactive at a forced 390px window, using the established pid-scoped
synthetic-click recipe:

- the logo menu, burger, `[T]`, chip, chip close button, and `[I]` remain
  clickable caption client areas;
- the chip opens the switcher and selecting a row switches documents;
- chip `x` closes the active document;
- `[T]` opens the tree over the canvas;
- `[I]` closes the tree and opens the inspector;
- a canvas press outside an open panel dismisses it;
- clicks inside a panel do not reach the canvas;
- dragging the window from 900px to 380px and back forks cleanly at both
  thresholds and preserves documents.

Capture a native-pixel screenshot of the 390px running window with
`scripts/capture-window.ps1`. Run the web build under a headless Playwright
probe and check the console for panics after exercising the layout fork.
