# Atlas restyle — design spec

Restyle the Svelte web app (`packages/web`) into the **Atlas** light-HUD design
language defined by the `docs/design/hud-*-mock.html` mocks. This spec is the
approved design; a separate implementation plan is written from it.

## Locked decisions (context)

- **Tokens**: pure CSS (custom-prop vars + component classes). Drop Tailwind
  incrementally — every restyled surface is fully converted off Tailwind.
  Tailwind stays installed until ALL surfaces (this pass + later passes) are
  converted, then removed in a final cleanup commit.
- **Scope this pass** (covered surfaces): nodes (ClassifierBox / GenericNode /
  OkfNode + **all** `Uml*Node` types — the classifier-shaped ones plus the
  non-classifier shapes `UmlNoteNode` / `UmlActorNode` / `UmlUseCaseNode` /
  `UmlPackageNode`), `inspector/*`, tags/stereotype rows, edges, buttons,
  menus/toolbars. **Out** for later passes: dialogs (except the modal shell),
  TopBar, Navigator, sequence view, Dock.
- **Font**: IBM Plex Sans (UI) + IBM Plex Mono (node identifiers / code); drop
  Source Sans 3. Bundle via `@fontsource/ibm-plex-sans` + `@fontsource/ibm-plex-mono`.
- **Light-only** (dark deferred — see `atlas-theme-decision` memory).
- **Modal shell template** = `hud-node-design-mock.html`.

---

## Section 1 — Token layer (`packages/web/src/atlas.css`)

New file, imported atop `app.css`. Defines:

- **Accent as an rgb triple** — `--accent: 20, 150, 220;` so `rgba(var(--accent),α)`
  works and a node can self-theme by stereotype via inline `style="--accent:…"`.
- `--danger: 235, 70, 120;` (also a triple).
- **Surfaces**: `--canvas-bg`, `--panel-fill`, `--ink`, `--ink-dim`, `--ink-faint`,
  `--hair`.
- **Geometry**: `--round: 0`, `--round-chip: 2px`, `--bw: 1.5px`, `--glow: .4`.
- **Type**: `--font-ui` (IBM Plex Sans stack), `--font-mono` (IBM Plex Mono stack).

Charcoal ink `#26313f` / `#212a34`, canvas `#eef2f7`, hairline
`rgba(var(--accent),.14–.22)` per the mocks.

---

## Section 2 — Material component classes (`packages/web/src/atlas-components.css`)

New file, imported after `atlas.css`. Tokens live in `atlas.css`; classes here.

### 2.0 The shared HUD material — one recipe, reused everywhere

Three stacked ingredients (pane, menu, node, button all share them):

**a) Frosted fill** — white gradient over faint accent tint:
```css
background:
  linear-gradient(180deg, rgba(255,255,255,.95), rgba(255,255,255,.82)),
  rgba(var(--accent), .06);
```
Knobbed via `--frost-top` / `--frost-bot` / `--frost-tint` so surfaces differ by
knob, not by copied recipe (node `.94→.80` / `.10`, button `.92→.74` / `.10`).

**b) Source-bright frame** — masked gradient border (no real `border`), bright
accent top → dimmer bottom:
```css
.hud-frame::before {
  content:""; position:absolute; inset:0; padding:var(--bw);
  background: linear-gradient(150deg, rgba(var(--accent),.95), rgba(var(--accent),.5));
  border-radius: inherit;
  -webkit-mask: linear-gradient(#000 0 0) content-box, linear-gradient(#000 0 0);
          mask: linear-gradient(#000 0 0) content-box, linear-gradient(#000 0 0);
  -webkit-mask-composite: xor; mask-composite: exclude;
  pointer-events:none;
}
```
`--bw` = 1.5px (S1). (This is why `--accent` is a triple.)

**c) Low glow** — depth shadow + accent bloom scaled by `--glow`:
```css
box-shadow:
  0 12px 30px rgba(40,70,110,.20),
  0 0 calc(14px * var(--glow)) rgba(var(--accent), calc(.16 * var(--glow)));
```
Panels/menus glow `.16`, nodes `.18` (softer depth `0 8px 22px …14`), buttons `.22`.

**Decision (approved): `.hud-surface` + knob-modifiers.** `.hud-surface` bundles
a/b/c square (`--round:0`); `.hud-surface--node` / `--btn` override only the knobs.
No recipe is duplicated (mocks duplicate only because they're standalone files).

### 2.1 Header block (`.hud-head`)
Flex row: mono uppercase `«stereo»` in accent + charcoal name, accent hairline
bottom (`rgba(var(--accent),.22)`). Node variants: `.hdr-band` (hairline only) vs
`.hdr-fill` (`rgba(var(--accent),.12)` fill).

### 2.2 Button (`.hud-btn`) — press ripple
Shared material + **two** frame pseudos: `::before` thin resting frame,
`::after` thick solid accent frame revealed by `clip-path: circle()` radially
from click point (`--ox`/`--oy` set by JS), plus a `glowpulse` flare that
decays. Mono uppercase label, `letter-spacing:.22em`. Press = `translateZ` down,
no tilt.

### 2.3 Segmented (`.seg`)
Accent-hairline box, square (3px), white cells; `.on` = solid `rgb(var(--accent))`
+ white text. Stretch variant = equal `flex:1` cells.

### 2.4 Toggle (`.toggle`)
28×18 square track, square 14px knob. Off = white + accent hairline. On = solid
accent fill + small accent glow; knob slides +8px, goes white.

### 2.5 Chips
- **Guillemet chip** (`.chip.slant`): square 2px, `rgba(var(--accent),.20)` fill,
  `«…»` via `::before`/`::after` in accent, mono. Hover reveals ✕ → `--danger` on
  hover. *(`tag-remove-uses-command-wheel` memory: radial menu is the eventual
  removal UX; ✕ stays this pass.)*
- **Column chip** (`.col`): 20px pill, off = white/dim, `.on` = accent wash +
  accent text, `.lock` = dashed inert.

### 2.6 Menu row (`.row`)
Rest transparent → hover soft accent wash (`.12`) + shortcut warms to accent →
active = white bg + thin masked accent frame + small glow (emissive press).
`.danger` variant swaps hue to `--danger` through all states.

---

## Section 3 — Per-surface mapping + conversion order

### 3.1 Conversion order (bottom-up; each item = one commit, each fully off Tailwind)

1. **Foundation** — `atlas.css` (S1) + `atlas-components.css` (S2); fonts swapped
   in `app.css` (drop `@fontsource-variable/source-sans-3`, add
   `@fontsource/ibm-plex-sans` + `@fontsource/ibm-plex-mono`). No surface
   converted yet — just makes the kit importable.
2. **Node leaves** — `StereotypeRow`, `AttributeRow`, `RowsCompartment` row.
3. **ClassifierBox** (keystone) — frosted node material + self-theming.
   GenericNode / OkfNode / all `Uml*Node` ride on it.
4. **Node badge** — `GenericNode` type pill → square chip.
5. **Edges** — `RelEdge` (+ `AnchorEdge`) stroke/label colors → tokens;
   `canvas.css` selection ring → accent frame + glow.
6. **Inspector** — `InspectorPanel` shell → `.hud-surface`; kind badge, buttons,
   ElementPicker → tokens.
7. **Toolbars/menus** — `SelectionToolbar` pill → `.hud-surface` + `.hud-btn` +
   `.row.danger`; any context menu → `.menu`/`.row`.
8. **Non-classifier UML nodes** — the shapes that don't ride ClassifierBox:
   `UmlUseCaseNode` (ellipse → `.hud-surface--node` with `border-radius:50%`),
   `UmlNoteNode` (folded-corner → bespoke two-layer clip-path frame + drop-shadow
   glow), `UmlActorNode` (stickman glyph → accent SVG strokes + drop-shadow glow),
   `UmlPackageNode` (tabbed folder → ClassifierBox body + a frosted accent tab).
   Each self-themes via inline `--accent` (`hexToTriple`) like the classifier
   nodes; all fully off Tailwind + hex.

Rationale: node surfaces render under the solver's measured sizing, so prove
them first; edges depend on node geometry; panels/toolbars are independent
chrome, last.

### 3.2 Mapping (current → atlas)

| File | Current | Atlas |
|---|---|---|
| `ClassifierBox` | `bg-white border-[1.5px] border-[#d8dee8] rounded-xl shadow-…` + `boxStyle` hex header | `.hud-surface--node` (`--round:0`); per-node `style="--accent:<triple>"` |
| `boxStyle` header | `border-top-color:hex; border-top-width:4px`, `bg color-mix` | `.hdr-band` (accent hairline) / `.hdr-fill` (`rgba(var(--accent),.12)`) |
| `StereotypeRow` | slate `«kw»` centered | `.hud-head .stereo` — accent mono uppercase |
| `AttributeRow` | slate rows, `text-slate-400 font-mono` | node `.row`: `.vis` accent, `.ty` `--ink-faint` mono |
| `GenericNode` badge | `rounded-full bg-[#94a3b8] text-white` | square chip, accent wash |
| `RelEdge` | `stroke:#64748b` / sel `#1e88e5`; label bg white | `stroke:rgb(var(--ink-faint))` / sel `rgb(var(--accent))`; label = frost |
| `canvas.css` sel | ring `#1e88e5` + halo `#e6f1fb` | accent masked frame + `--glow` bloom |
| `InspectorPanel` | `bg-white border-[#d8dee8] rounded-xl shadow-…` | `.hud-surface` (panel knobs) |
| Inspector kind badge | `bg-[#e6f1fb] text-[#1e88e5]` | `rgba(var(--accent),.12)` + `rgb(var(--accent))` |
| `SelectionToolbar` | pill `bg-white border rounded-xl`; btns `#1e88e5`/`#dc2626` | `.hud-surface` + `.hud-btn`; danger = `--danger` |
| `UmlUseCaseNode` | ellipse `border-[#c8d2e0] bg-white shadow-…` | `.hud-surface--node` + `border-radius:50%` (frame/glow inherit the ellipse) |
| `UmlNoteNode` | folded `clip-path`, cream `bg-[#fffdf3] border-[#e3d9a8]` | frost fill + two-layer clip-path accent frame + `drop-shadow` glow; accent crease |
| `UmlActorNode` | SVG stickman `stroke:#334155`; `text-slate-800` | strokes `rgb(var(--accent))` + `drop-shadow` glow; name Plex Mono uppercase |
| `UmlPackageNode` | tab `border-[#d8dee8] bg-white` + ClassifierBox | ClassifierBox body (Atlas) + frosted accent-hairline tab |

### 3.3 Cross-cutting decisions (approved)

**A. Node self-theming (approved).** `stereotypeStyle` yields **hex** (`st.header`,
`overrideHeader`); Atlas needs an `--accent` **triple**. A tiny JS `hexToTriple()`
at render emits `style="--accent:<r,g,b>"` on the node wrapper — profile data
untouched, no schema change, default blue triple when no stereotype. Render as
close to the HUD mock as possible.

**B. Node identifier font (approved — mock fidelity, with a banked caveat).**
Node name + attribute names render **IBM Plex Mono uppercase** per the mock (the
look we want). **Known caveat**: `waml::solve::sizing` measures node text in Plex
**Sans**, so hulls are sized in Sans while identifiers draw in Mono → widths
drift (possible clip or slack). Accepted for this pass; a later pass should teach
the solver to measure Plex Mono for identifier text. Plex Mono is used for node
identifiers + code fragments (attribute types, etc.); Plex Sans is the UI font.

**C. Edge/label colors (approved).** `RelEdge` builds SVG styles as JS strings,
not classes. Embed CSS vars directly in the string (`stroke:rgb(var(--accent))`,
label bg = frost) — vars resolve on the SVG element, so no duplicate JS color
constants.

---

## Section 4 — Tailwind-drop tracking + vite verify loop

### 4.1 Tailwind-drop ledger

Tailwind (`^3.4.0`) stays installed this whole pass — later passes still use it.
A surface is **done** only when fully off Tailwind. Ledger (this pass):

| Surface | Tailwind-free | Mock parity |
|---|---|---|
| foundation (atlas.css / components / fonts) | ☐ | — |
| StereotypeRow / AttributeRow / RowsCompartment | ☐ | ☐ |
| ClassifierBox | ☐ | ☐ |
| GenericNode badge | ☐ | ☐ |
| RelEdge + canvas.css select | ☐ | ☐ |
| InspectorPanel | ☐ | ☐ |
| SelectionToolbar | ☐ | ☐ |
| UmlUseCaseNode / UmlNoteNode / UmlActorNode / UmlPackageNode | ☐ | ☐ |

**Mechanical "Tailwind-free" check** per converted file (zero hits = clean):
```bash
rg -n 'class="[^"]*(bg-|text-|border-|rounded-|shadow-|flex|grid|px-|py-|gap-|w-\[|h-\[)' <file>
rg -n '@apply' <file>
```
Converted surfaces use `class="hud-… atlas-…"` + scoped `<style>` only.

Ledger 100% across **this pass and all later passes** → **final cleanup commit**
removes `tailwindcss` / `postcss` deps + the three `@tailwind` directives from
`app.css`. **Not this pass.**

### 4.2 Vite verify loop (live devserver)

`pnpm --filter @waml/web dev` (= `vite`) runs hot-reloading for the whole pass.
Per surface commit:

1. Restyle surface, save → HMR repaints.
2. No GPU vision: headless Edge screenshot the running app at the surface's state
   → `Read` the png → compare against the matching `docs/design/hud-*-mock.html`
   screenshot.
3. Gate before commit: `pnpm --filter @waml/web check` (svelte-check) +
   `pnpm --filter @waml/web build` + the 4.1 grep. Green + visual parity = commit.

Mock ↔ surface pairing for the visual diff:
- ClassifierBox / rows / badge → `hud-node-mock.html` + `hud-node-design-mock.html` preview
- `RelEdge` → `hud-edges-mock.html`
- `InspectorPanel` → `hud-inspector-mock.html`
- `SelectionToolbar` → `hud-button-mock.html` + `hud-menu-mock.html`
- `UmlUseCaseNode` / `UmlNoteNode` / `UmlActorNode` / `UmlPackageNode` →
  `hud-icons-mock.html` (shape/glyph treatment) + `hud-node-mock.html` (material)

Seeding app state to reach each surface (a node with stereotype + attributes, a
selected edge, inspector open) — the plan step scripts or hand-drives it via the
devserver.

### 4.3 Out-of-scope guard

Dialogs (except modal shell), TopBar, Navigator, sequence view, Dock stay
Tailwind this pass — the ledger doesn't gate on them; they block only the final
removal commit in a later pass.
