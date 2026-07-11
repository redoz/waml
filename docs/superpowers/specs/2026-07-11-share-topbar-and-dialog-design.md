# Share in Top Bar + Share Dialog

**Date:** 2026-07-11
**Product:** Model Canvas (`packages/web`, React + React Flow + TypeScript)
**Scope:** 3 of 5 UI-change specs. Independent. Frees the right rail's Share so
the right-edge-flags spec can delete the rail.

## Context

Share currently lives in the **right icon rail** (`RightRail.tsx:7-10`, the
`share` entry) and opens a **SharePanel** (`packages/web/src/components/rail/SharePanel.tsx`)
inside the sliding **ModelSheet** (`Canvas.tsx:600-606`). SharePanel shows a
read-only share URL + Copy (`SharePanel.tsx:26-40`) and an "Export as image"
button (`SharePanel.tsx:43-49`). The share URL is built by `buildShareUrl`
(`packages/web/src/share/url.ts`). Because `modal={panel.active !== "inspect"}`
(`Canvas.tsx:574`), the Share sheet already dims the background like a modal.

The top bar has an **Export** dropdown (`TopBar.tsx:124-149`) with "OKF (Markdown)"
→ `onExport` and "Image (SVG)" → `onExportSvg`.

## Goal

- Move **Share** into the **top bar, immediately right of Export**, as a first-class
  button.
- Clicking opens a real modal **Share dialog** (not the side sheet) containing the
  share link *and* a **Share as image** flow that renders the diagram to **PNG**
  and lets the user copy it or save it to disk.

## Current state (concrete)

- Rail Share entry: `RightRail.tsx:7-10`, rendered `RightRail.tsx:22-29`; opens the
  share panel via `onOpen("share")`.
- SharePanel: `SharePanel.tsx` (URL + Copy + "Export as image").
- Share URL: `share/url.ts` `buildShareUrl`, used `Canvas.tsx:602`.
- Export button + menu: `TopBar.tsx:124-149`; handlers `handleExport` /
  `handleExportSvg` (`Canvas.tsx:446-448`).
- SVG export path (reuse target for PNG): `onExportSvg` / "Image (SVG)".

## Changes

### Top bar Share button

- Add a **Share** button in `TopBar.tsx` directly to the right of the Export
  dropdown. Style consistent with Export.
- Clicking sets state to open the Share dialog (new modal; see below). Remove the
  rail-driven share path from `Canvas.tsx` (`onOpen("share")` usage) — the rail
  itself is deleted in the right-edge-flags spec, but this spec stops routing
  Share through it.

### Share dialog (modal)

New modal component (e.g. `components/share/ShareDialog.tsx`) replacing the
sheet-hosted SharePanel. Content:

1. **Share link** — read-only input with the URL from `buildShareUrl` + a **Copy**
   button (reuse SharePanel's copy behavior). Keep the "named sharing" header text.
2. **Share as image** — a section that renders the current diagram to PNG and
   offers:
   - **Copy image** — `navigator.clipboard.write([new ClipboardItem({ "image/png": blob })])`.
   - **Save to disk** — download anchor (`a.download = "<diagram>.png"`).
   - Show the rendered preview (or a thumbnail) so the user can also right-click →
     copy/save it manually as a fallback where the Clipboard API is unavailable
     (e.g. Firefox).

### PNG rendering

- **Approach (recommended, no new dependency):** reuse the existing SVG export
  (`onExportSvg` path) to produce the diagram SVG string, load it into an
  `Image` from a data URL, draw onto an `HTMLCanvasElement`, then
  `canvas.toBlob(blob => ..., "image/png")`.
  - **Gotcha:** fonts and CSS referenced by the SVG must be inlined into the SVG
    markup or the raster comes out unstyled. Verify the existing SVG exporter
    inlines styles; if not, inline them in the rasterization step.
- **Alternative:** `html-to-image` `toPng(reactFlowViewportEl)` (React Flow's
  recommended approach) handles style inlining but adds a dependency. Fall back to
  this only if inlining the SVG proves fragile.
- Clipboard write requires a secure context and a user gesture (the button click
  satisfies the gesture). Handle unsupported browsers by disabling **Copy image**
  and relying on **Save to disk** + right-click.

### Remove old Share sheet path

- Delete `SharePanel.tsx` (its URL/copy logic migrates into the dialog; its
  "Export as image" is superseded by the PNG flow).
- Remove the `"share"` case from the ModelSheet host and `useRightPanel.ts`
  (`RightPanelId` becomes `"inspect"` only). Note: the ModelSheet + rail continue
  to host **Inspect** until the right-edge-flags spec reworks it.

## Edge cases

- **Empty diagram:** disable **Share as image** when there is nothing to render.
- **Clipboard unsupported:** hide/disable **Copy image**, keep Save + right-click.
- **Large diagrams:** rasterize at the diagram's natural bounds (fit content), cap
  max dimension to avoid enormous canvases; note the cap to the user if applied.

## Out of scope

- Deleting `RightRail` / reworking Inspect (spec: right-edge-flags) — this spec
  only removes Share from the rail routing.

## Testing

- Component: top-bar Share button sits right of Export and opens the dialog;
  dialog shows the URL and Copy works.
- Unit: SVG→PNG produces a non-empty `image/png` blob for a sample diagram.
- Behavior: Copy image disabled when Clipboard API absent; Save triggers a
  download with the diagram name.
