# Replace Brand Logo with UAML Wordmark

**Date:** 2026-07-11
**Product:** Model Canvas (`packages/web`, React + React Flow + TypeScript)
**Scope:** 6 of 6 UI-change specs. Independent; small.

## Context

The top bar's brand cluster (`packages/web/src/components/TopBar.tsx:61-76`) is:

- an inline SVG `LOGO` const (`TopBar.tsx:21-42`) — the OWOX blue-gradient mark
  (512×512, two gradient paths),
- wrapped in `<a href="https://owox.com" title="OWOX — owox.com" aria-label="OWOX
  — owox.com">` (`TopBar.tsx:65-74`),
- followed by `<span>Model Canvas</span>` (`TopBar.tsx:75`).

We are swapping the OWOX mark for the new **UAML wordmark** (source SVG:
`C:\Users\redoz\Downloads\uaml.svg` — four letter paths, `viewBox="-20 -20 440
140"`, originally filled burnt orange `#CC5500`).

## Goal

- Replace the OWOX `LOGO` SVG with the UAML wordmark.
- Keep the **"Model Canvas"** text beside it.
- Point the brand link at **https://github.com/redoz/uaml**.
- Render the wordmark with **`currentColor`** (inherits text color; adapts to
  theme/hover) instead of the hardcoded `#CC5500`.

## Changes (all in `TopBar.tsx`)

### Replace the `LOGO` const (`TopBar.tsx:21-42`)

Inline the wordmark as JSX, converted from the source SVG: drop the `<style>
.letter` rule and the `#CC5500` fill, apply `fill="currentColor"` on the group,
and size to match the current 24px-tall logo (aspect ratio ≈ 440:140 ≈ 3.14, so
height 24 ⇒ width ≈ 75). Let width scale from height via the `viewBox`.

```tsx
const LOGO = (
  <svg viewBox="-20 -20 440 140" height={24} fill="currentColor"
       xmlns="http://www.w3.org/2000/svg" role="img" aria-label="UAML">
    <g>
      {/* U */}
      <path d="M 0,0 H 25 V 75 H 55 V 0 H 80 V 85 L 65,100 H 15 L 0,85 Z" transform="translate(0, 0)" />
      {/* A */}
      <path fillRule="evenodd" d="M 0,100 V 15 L 15,0 H 65 L 80,15 V 100 H 55 V 65 H 25 V 100 Z M 25,25 H 55 V 40 H 25 Z" transform="translate(100, 0)" />
      {/* M */}
      <path d="M 0,100 V 0 H 25 L 50,40 L 75,0 H 100 V 100 H 75 V 45 L 50,75 L 25,45 V 100 Z" transform="translate(200, 0)" />
      {/* L */}
      <path d="M 0,0 H 25 V 75 H 80 V 85 L 65,100 H 15 L 0,85 Z" transform="translate(320, 0)" />
    </g>
  </svg>
);
```

Notes:
- JSX requires `fillRule` (not `fill-rule`) — already applied on the A path.
- No explicit `width` so the intrinsic `viewBox` ratio drives it off `height`.

### Update the brand link (`TopBar.tsx:65-74`)

- `href` → `https://github.com/redoz/uaml`.
- `title` / `aria-label` → `"UAML — GitHub"` (or similar).
- Keep `target="_blank"` + `rel="noreferrer"` and the existing hover/rounded
  classes.

### Keep the text

- Leave `<span>Model Canvas</span>` (`TopBar.tsx:75`) unchanged.

## Considerations

- **currentColor:** the brand cluster's default text color is the top bar's dark
  text, so the wordmark renders dark; `hover:opacity-80` on the anchor still
  applies. If a dark theme is ever added, the wordmark follows text color for
  free. The burnt-orange brand color is intentionally dropped (per decision).
- **Asset location:** the wordmark is inlined into `TopBar.tsx` (matching the
  current pattern) rather than imported as a file, so no bundler asset wiring is
  needed. Do **not** keep the OWOX gradient defs.
- **Favicon / other logo uses:** out of scope — this spec only touches the top-bar
  brand. Flag any other OWOX-logo occurrences found during implementation but do
  not change them here.

## Out of scope

- Favicon, README, meta tags, or any non-top-bar branding.
- Renaming the product from "Model Canvas".

## Testing

- Component: top bar renders the UAML wordmark; the anchor `href` is the GitHub
  repo; `aria-label` reads UAML; "Model Canvas" text still present.
- Visual: wordmark sits at ~24px height, aligned with the text, inherits text
  color, dims on hover.
- Regression: no remaining reference to the old OWOX gradient ids
  (`topbar-g0` / `topbar-g1`).
