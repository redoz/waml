# Atlas Restyle (waml web app) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Restyle the `packages/web` Svelte app into the **Atlas** light-HUD design language from `docs/design/hud-*-mock.html`, converting nodes, edges, inspector, and toolbars fully off Tailwind onto a pure-CSS token + component-class kit.

**Architecture:** Two new global stylesheets — `atlas.css` (custom-property tokens) and `atlas-components.css` (the shared HUD "material" recipe + component classes) — are imported atop `app.css`. Every in-scope surface is converted to `hud-*` / `node-*` / `menu-*` classes plus small scoped `<style>` blocks, one surface per commit, bottom-up (leaves → keystone → chrome). Nodes self-theme by writing a per-node `--accent` rgb **triple** (from a `hexToTriple()` helper) inline; edges embed CSS vars directly in their SVG style strings so no JS color constants duplicate the tokens.

**Tech Stack:** Svelte 5 (runes), Vite 5, `@xyflow/svelte` 1, TypeScript, `@fontsource/ibm-plex-sans` + `@fontsource/ibm-plex-mono`. Tailwind `^3.4.0` stays installed (removed in a later pass, NOT here).

## Global Constraints

Every task's requirements implicitly include this section. Values are copied verbatim from the spec (`docs/superpowers/specs/2026-07-18-atlas-restyle-design.md`).

- **Pure CSS tokens only** — custom-property vars in `atlas.css`, component classes in `atlas-components.css`. No CSS-in-JS framework, no new deps beyond the two fontsource packages.
- **Every converted surface fully off Tailwind.** Tailwind stays installed and its `@tailwind` directives stay in `app.css` this whole pass. **Do NOT remove `tailwindcss`/`postcss` deps or the `@tailwind` directives in this plan** — that is a later final-cleanup pass gated on surfaces this plan does not touch.
- **Fonts:** IBM Plex Sans = UI font (`--font-ui`); IBM Plex Mono = node identifiers + code fragments (`--font-mono`). Drop `@fontsource-variable/source-sans-3` entirely.
- **Light-only.** No dark-mode variables or `prefers-color-scheme` blocks.
- **`--accent` is an rgb TRIPLE** (`20, 150, 220`), never a hex. This is what makes `rgba(var(--accent), α)` and the masked-frame `rgb(var(--accent))` work. Nodes self-theme via inline `style="--accent:<r,g,b>"` produced by `hexToTriple()`; default is the blue triple `20, 150, 220` when a node has no stereotype color.
- **`--danger` is also a triple** (`235, 70, 120`).
- **Node identifiers render IBM Plex Mono, uppercase**, per the mock. BANKED CAVEAT (accepted this pass, do not fix here): `waml::solve::sizing` measures node text in Plex **Sans**, so hulls are sized in Sans while identifiers draw in Mono → widths may clip or run slack. A later pass teaches the solver to measure Plex Mono. Do not touch the Rust solver.
- **Edge/label colors via CSS-var-in-style-string** (`stroke:rgb(var(--accent))`, `stroke:rgb(var(--ink-faint))`); the vars resolve on the SVG element. No duplicate JS color constants (delete the old `#1e88e5`/`#64748b`/`#94a3b8` literals as you convert).
- **Square corners** — `--round: 0`. Chips use `--round-chip: 2px`. Border weight `--bw: 1.5px`. Canvas glow `--glow: .4`.
- **Out of scope — DO NOT convert:** dialogs (except the modal shell, not touched here), TopBar, Navigator, sequence view, Dock. Leave them on Tailwind; they do not gate this plan's ledger.

---

## File Structure

**New files (created in Task 1):**
- `packages/web/src/atlas.css` — token layer (Section 1 of spec). All `--*` custom props on `:root`.
- `packages/web/src/atlas-components.css` — the shared HUD material recipe + every component class (Section 2 of spec): `.hud-surface` (+ `--node`/`--btn` knob modifiers), node card classes, `.hud-btn`, `.seg`, `.toggle`, chips, `.hud-menu`/`.menu-row`, keyframes.

**New file (created in Task 7):**
- `packages/web/src/lib/hudPress.ts` — a tiny Svelte `use:` action that sets `--ox`/`--oy` from the click point and toggles the `.down` class to fire the button ripple. JS behaviour only; the CSS lives in `atlas-components.css`.

**Modified files:**
- `packages/web/src/app.css` — swap font `@import`s, add the two new stylesheet `@import`s, set base body font/ink. (Task 1)
- `packages/web/package.json` — drop `@fontsource-variable/source-sans-3`, add `@fontsource/ibm-plex-sans` + `@fontsource/ibm-plex-mono`. (Task 1)
- `packages/web/src/components/canvas/nodes/types.ts` — `NODE_FONT` → Plex Mono stack; add `hexToTriple()`. (Task 1)
- `.../nodes/StereotypeRow.svelte`, `AttributeRow.svelte`, `RowsCompartment.svelte` — Task 2.
- `.../nodes/ClassifierBox.svelte`, `NodePorts.svelte`, `GenericNode.svelte` (badge) — Tasks 3 & 4. (`OkfNode` and the classifier-shaped `Uml*Node` types ride on ClassifierBox unchanged. `UmlNoteNode`/`UmlActorNode`/`UmlUseCaseNode`/`UmlPackageNode` carry their own distinct shapes + Tailwind and are converted in Task 8; `UmlPackageNode`'s box body rides ClassifierBox while its tab is converted in Task 8.)
- `.../canvas/RelEdge.svelte`, `AnchorEdge.svelte`, `.../canvas/canvas.css` — Task 5.
- `.../inspector/InspectorPanel.svelte`, `ElementPicker.svelte` — Task 6.
- `.../canvas/SelectionToolbar.svelte` — Task 7.
- `.../nodes/UmlNoteNode.svelte`, `UmlActorNode.svelte`, `UmlUseCaseNode.svelte`, `UmlPackageNode.svelte` — Task 8.

---

## Standing prerequisites (read once before Task 2)

**Live devserver (start ONCE in Task 1, keep running the whole pass):**
```
pnpm --filter @waml/web dev
```
This is `vite` — HMR repaints every save at `http://localhost:5173`. Do not restart it per task.

**Headless screenshot helper (no GPU vision).** After each surface, screenshot the running app AND the paired mock, then `Read` both pngs and compare. PowerShell (scratchpad dir is session-specific — substitute `<SCRATCH>` = `C:\Users\redoz\AppData\Local\Temp\claude\C--dev-waml\e98f19f0-a704-461e-aefe-0087816f2f56\scratchpad`):
```powershell
& "${env:ProgramFiles(x86)}\Microsoft\Edge\Application\msedge.exe" `
  --headless=new --disable-gpu --hide-scrollbars --window-size=1400,900 `
  --screenshot="<SCRATCH>\app.png" "http://localhost:5173"
& "${env:ProgramFiles(x86)}\Microsoft\Edge\Application\msedge.exe" `
  --headless=new --disable-gpu --hide-scrollbars --window-size=1400,900 `
  --screenshot="<SCRATCH>\mock.png" `
  "file:///C:/dev/waml/.claude/worktrees/hud-button/docs/design/hud-node-mock.html"
```
Then `Read <SCRATCH>\app.png` and `Read <SCRATCH>\mock.png` and eyeball parity. For app states that need seeding (a node with a stereotype + attributes, a selected edge, an open inspector), hand-drive the running app to that state in a normal (non-headless) Edge window first, or use the app's own seed/sample document, then screenshot. The mock ↔ surface pairing per task is stated in each task.

**Mechanical Tailwind-free gate (run per converted file — ZERO hits required):**
```
rg -n 'class="[^"]*(bg-|text-|border-|rounded-|shadow-|flex|grid|px-|py-|gap-|w-\[|h-\[)' <file>
rg -n '@apply' <file>
```

**Pre-commit gate (every task):**
```
pnpm --filter @waml/web check
pnpm --filter @waml/web build
```
Both must pass. `check` is `svelte-check`; `build` runs `svelte-check` + `vite build`.

---

## Task 1: Foundation — tokens, material kit, fonts

**Files:**
- Create: `packages/web/src/atlas.css`
- Create: `packages/web/src/atlas-components.css`
- Modify: `packages/web/src/app.css:1-7`
- Modify: `packages/web/package.json:15` (deps block)
- Modify: `packages/web/src/components/canvas/nodes/types.ts:8` (NODE_FONT + add hexToTriple)

**Interfaces:**
- Produces (consumed by Tasks 2-7):
  - CSS classes: `.hud-surface`, `.hud-surface--node`, `.hud-surface--btn`, `.hud-node`, `.hud-node__body`, `.node-hdr`, `.node-hdr--band`, `.node-hdr--fill`, `.node-stereo`, `.node-name`, `.node-name--abstract`, `.node-comp`, `.node-row`, `.node-row__vis`, `.node-row__name`, `.node-row__ty`, `.node-summary`, `.node-more`, `.node-badge`, `.node-port`, `.hud-btn`, `.hud-btn--sm`, `.seg`, `.seg__btn`, `.toggle`, `.chip-slant`, `.col-chip`, `.hud-menu`, `.menu-row`, `.menu-row--danger`, `.menu-sep`, `.menu-kbd`, `.hud-ctrl`, `.hud-field`. Keyframes `reveal`, `glowpulse`, `nodeglow`.
  - Tokens on `:root`: `--accent`, `--danger`, `--canvas-bg`, `--panel-fill`, `--ink`, `--ink-dim`, `--ink-faint`, `--hair`, `--round`, `--round-chip`, `--bw`, `--glow`, `--font-ui`, `--font-mono`. NOTE: `--ink-faint` is an rgb **triple** (used via `rgb(var(--ink-faint))` in edge strings); `--ink`, `--ink-dim` are hex.
  - TS: `export function hexToTriple(hex?: string): string` → `"r, g, b"`, default `"20, 150, 220"`. `export const NODE_FONT` = Plex Mono stack.

- [ ] **Step 1: Create `atlas.css` (token layer)**

```css
/* Atlas token layer — Section 1 of the atlas-restyle spec.
   Light-only. --accent / --danger / --ink-faint are rgb TRIPLES so
   rgba(var(--accent), α) and rgb(var(--accent)) both work. */
:root {
  color-scheme: light;

  /* accent + danger as rgb triples (NOT hex) */
  --accent: 20, 150, 220;
  --danger: 235, 70, 120;

  /* surfaces */
  --canvas-bg: #eef2f7;
  --panel-fill: #ffffff;
  --ink: #212a34;          /* charcoal — node name / body text */
  --ink-dim: #3a4552;      /* dimmer body text */
  --ink-faint: 123, 135, 151;  /* TRIPLE (#7b8797) — type text, default edge stroke */
  --hair: rgba(var(--accent), .14);

  /* geometry */
  --round: 0;
  --round-chip: 2px;
  --bw: 1.5px;
  --glow: .4;

  /* type */
  --font-ui: "IBM Plex Sans", -apple-system, "Segoe UI", system-ui, sans-serif;
  --font-mono: "IBM Plex Mono", ui-monospace, "SF Mono", "Cascadia Code", Menlo, monospace;
}
```

- [ ] **Step 2: Create `atlas-components.css` (material + component classes)**

```css
/* Atlas material + component classes — Section 2 of the spec.
   Tokens live in atlas.css; this file is classes only. The one HUD
   material recipe (frost fill + masked source-bright frame + low glow)
   is defined ONCE on .hud-surface; surfaces differ only by knob overrides. */

/* ── 2.0 shared HUD material ─────────────────────────────── */
.hud-surface {
  /* knobs (panel defaults; node/btn override below) */
  --frost-top: .95;
  --frost-bot: .82;
  --frost-tint: .06;
  --depth-y: 12px;
  --depth-blur: 30px;
  --depth-a: .20;
  --bloom: .16;
  position: relative;
  border-radius: var(--round);
  background:
    linear-gradient(180deg, rgba(255, 255, 255, var(--frost-top)), rgba(255, 255, 255, var(--frost-bot))),
    rgba(var(--accent), var(--frost-tint));
  box-shadow:
    0 var(--depth-y) var(--depth-blur) rgba(40, 70, 110, var(--depth-a)),
    0 0 calc(14px * var(--glow)) rgba(var(--accent), calc(var(--bloom) * var(--glow)));
}
/* b) source-bright masked frame — the thin resting frame, always on */
.hud-surface::before {
  content: ""; position: absolute; inset: 0; padding: var(--bw);
  background: linear-gradient(150deg, rgba(var(--accent), .95), rgba(var(--accent), .5));
  border-radius: inherit;
  -webkit-mask: linear-gradient(#000 0 0) content-box, linear-gradient(#000 0 0);
          mask: linear-gradient(#000 0 0) content-box, linear-gradient(#000 0 0);
  -webkit-mask-composite: xor; mask-composite: exclude;
  pointer-events: none;
}
/* node knobs: softer depth, warmer bloom */
.hud-surface--node {
  --frost-top: .94; --frost-bot: .80; --frost-tint: .06;
  --depth-y: 8px; --depth-blur: 22px; --depth-a: .14; --bloom: .18;
}
/* button knobs: brighter tint + bloom, tight depth */
.hud-surface--btn {
  --frost-top: .92; --frost-bot: .74; --frost-tint: .10;
  --depth-y: 6px; --depth-blur: 18px; --depth-a: .14; --bloom: .22;
}

/* ── keyframes ───────────────────────────────────────────── */
@keyframes reveal {
  from { clip-path: circle(0%   at var(--ox, 50%) var(--oy, 50%)); }
  to   { clip-path: circle(150% at var(--ox, 50%) var(--oy, 50%)); }
}
@keyframes glowpulse {
  0%   { box-shadow: 0 2px 6px rgba(40, 70, 110, .16), 0 0 26px rgba(var(--accent), .50); }
  100% { box-shadow: 0 2px 6px rgba(40, 70, 110, .16), 0 0  5px rgba(var(--accent), .09); }
}
@keyframes nodeglow {
  0%   { box-shadow: 0 6px 16px rgba(40, 70, 110, .16), 0 0 26px rgba(var(--accent), .50); }
  100% { box-shadow: 0 6px 16px rgba(40, 70, 110, .16), 0 0  5px rgba(var(--accent), .09); }
}

/* ── node card ───────────────────────────────────────────── */
.hud-node { width: 230px; cursor: grab; user-select: none; color: var(--ink); font-family: var(--font-mono); }
.hud-node__body { position: relative; z-index: 1; }

.node-hdr { padding: 9px 12px 8px; }
.node-hdr--band { border-bottom: 1px solid rgba(var(--accent), .22); }
.node-hdr--fill { background: rgba(var(--accent), .12); border-bottom: 1px solid rgba(var(--accent), .22); }

.node-stereo {
  text-align: center;
  font: 600 10px/1.35 var(--font-mono);
  letter-spacing: .18em; text-transform: uppercase;
  color: rgb(var(--accent)); opacity: .85;
}
.node-name {
  padding: 3px 12px 8px; text-align: center;
  font: 700 13.5px/1.15 var(--font-mono);
  letter-spacing: .06em; text-transform: uppercase; color: var(--ink);
}
.node-name--abstract { font-style: italic; }

.node-comp { border-top: 1px solid rgba(var(--accent), .14); }
.node-row {
  display: flex; align-items: center; gap: 8px; padding: 5px 12px;
  font: 500 11.5px/1.6 var(--font-mono); color: var(--ink-dim);
  border-bottom: 1px solid rgba(var(--accent), .10);
}
.node-row:last-child { border-bottom: 0; }
.node-row__vis  { flex: 0 0 auto; color: rgb(var(--accent)); }
.node-row__name { flex: 1 1 auto; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.node-row__ty   { flex: 0 0 auto; font-size: 10.5px; color: rgb(var(--ink-faint)); white-space: nowrap; }

.node-summary { padding: 3px 12px 10px; text-align: center; font: 500 11px/1.4 var(--font-mono); color: rgb(var(--ink-faint)); }

.node-more {
  width: 100%; display: flex; align-items: center; justify-content: center; gap: 4px;
  padding: 5px 12px; border-top: 1px solid rgba(var(--accent), .10);
  font: 600 11px/1 var(--font-mono); letter-spacing: .04em;
  color: rgb(var(--accent)); background: transparent; border-left: 0; border-right: 0; border-bottom: 0; cursor: pointer;
}
.node-more:hover { background: rgba(var(--accent), .10); }

.node-badge {
  display: inline-block; margin: 8px 12px 0; padding: 2px 7px;
  border-radius: var(--round-chip);
  font: 700 10px/1.4 var(--font-mono); letter-spacing: .14em; text-transform: uppercase;
  color: rgb(var(--accent)); background: rgba(var(--accent), .12);
}

/* port nub on a node border (half in / half out) */
.node-port {
  width: 11px; height: 11px; border-radius: 50%;
  background: #fff; border: 2px solid rgb(var(--accent));
  opacity: 0; transition: opacity .12s; z-index: 10; transform: translate(-50%, -50%);
}

/* ── button (2.2) — .hud-surface .hud-surface--btn .hud-btn ─── */
.hud-btn {
  border: none; outline: none; -webkit-appearance: none; cursor: pointer; user-select: none;
  display: grid; place-items: center; padding: 12px 20px;
  color: var(--ink);
  font: 600 15px/1 var(--font-mono); letter-spacing: .22em; text-transform: uppercase;
  text-shadow: 0 1px 0 rgba(255, 255, 255, .7);
  --ox: 50%; --oy: 50%;
}
.hud-btn--sm { padding: 7px 12px; font-size: 12px; letter-spacing: .12em; }
/* thick accent frame, revealed radially from click point on press (JS toggles .down) */
.hud-btn::after {
  content: ""; position: absolute; inset: 0; padding: 2px;
  background: rgb(var(--accent)); border-radius: inherit;
  -webkit-mask: linear-gradient(#000 0 0) content-box, linear-gradient(#000 0 0);
          mask: linear-gradient(#000 0 0) content-box, linear-gradient(#000 0 0);
  -webkit-mask-composite: xor; mask-composite: exclude;
  clip-path: circle(0% at 50% 50%); pointer-events: none;
}
.hud-btn.down::after { animation: reveal 100ms ease-out forwards; }
.hud-btn.down { animation: glowpulse 420ms ease-out forwards; }
.hud-btn:disabled { cursor: not-allowed; opacity: .5; }

/* ── segmented (2.3) ─────────────────────────────────────── */
.seg { display: inline-flex; border: 1px solid rgba(var(--accent), .26); border-radius: 3px; overflow: hidden; }
.seg__btn {
  border: 0; background: #fff; cursor: pointer; padding: 7px 12px; height: 28px;
  font: 500 11px/1 var(--font-ui); color: rgb(var(--ink-faint));
  border-left: 1px solid rgba(var(--accent), .18);
}
.seg__btn:first-child { border-left: 0; }
.seg__btn.is-on { background: rgb(var(--accent)); color: #fff; }

/* ── toggle (2.4) — square 28×18 track, square 14px knob ──── */
.toggle {
  flex: 0 0 auto; position: relative; width: 28px; height: 18px; cursor: pointer;
  border-radius: 2px; background: #fff; border: 1px solid rgba(var(--accent), .35);
  transition: background .12s ease, border-color .12s ease, box-shadow .12s ease;
}
.toggle::after {
  content: ""; position: absolute; top: 0; bottom: 0; left: 2px; margin: auto 0;
  width: 14px; height: 14px; border-radius: 2px;
  background: transparent; border: 1px solid rgba(var(--accent), .55);
  transition: transform .12s ease, background .12s ease, border-color .12s ease;
}
.toggle.is-on { background: rgb(var(--accent)); border-color: rgb(var(--accent));
  box-shadow: 0 0 5px rgba(var(--accent), .35), 0 0 1px rgba(var(--accent), .35); }
.toggle.is-on::after { transform: translateX(8px); background: #fff; border-color: #fff; }

/* ── chips (2.5) ─────────────────────────────────────────── */
.chip-slant {
  display: inline-flex; align-items: center; gap: 5px; height: 22px; padding: 0 9px;
  border: 0; border-radius: 2px; color: #22303c; background: rgba(var(--accent), .20);
  font: 500 11.5px/1 var(--font-mono);
}
.chip-slant::before { content: "\00ab"; color: rgb(var(--accent)); opacity: .7; margin-right: -3px; }
.chip-slant::after  { content: "\00bb"; color: rgb(var(--accent)); opacity: .7; margin-left: -3px; }
.chip-slant:hover { background: rgba(var(--accent), .30); }

.col-chip {
  display: inline-flex; align-items: center; height: 20px; padding: 0 9px;
  border-radius: 2px; cursor: pointer;
  border: 1px solid rgba(var(--accent), .30); background: #fff; color: rgb(var(--ink-faint));
  font: 600 10px/1 var(--font-ui);
  transition: background .1s ease, color .1s ease, border-color .1s ease;
}
.col-chip.is-on { background: rgba(var(--accent), .15); border-color: rgba(var(--accent), .48); color: #22303c; }
.col-chip.is-lock { border-style: dashed; cursor: default; color: rgb(var(--ink-faint)); opacity: .8; }

/* ── inspector-style control field + focus flare ─────────── */
.hud-field { display: grid; grid-template-columns: 78px 1fr; align-items: center; gap: 10px; margin: 9px 0; }
.hud-field > label { font: 500 11px/1.3 var(--font-ui); color: #6a7686; }
.hud-ctrl {
  position: relative; width: 100%; display: flex; align-items: center;
  background: #fff; border: 1px solid rgba(var(--accent), .26); border-radius: 3px; padding: 6px 9px;
  font: 500 12.5px/1 var(--font-ui); color: var(--ink);
}
.hud-ctrl input { border: 0; outline: 0; background: transparent; width: 100%; font: inherit; color: inherit; padding: 0; }
.hud-ctrl:focus-within {
  border-color: rgb(var(--accent));
  box-shadow: 0 0 0 1px rgb(var(--accent)), 0 0 5px rgba(var(--accent), .30), 0 0 1px rgba(var(--accent), .30);
}

/* ── menu (2.6) — .hud-surface + rows ────────────────────── */
.hud-menu { padding: 6px; }
.menu-row {
  position: relative; display: flex; align-items: center; gap: 11px;
  padding: 8px 11px; border-radius: 2px; cursor: default; user-select: none;
  font: 500 12.5px/1 var(--font-ui); color: #2c3744; white-space: nowrap;
  transition: background .08s ease, color .08s ease, box-shadow .08s ease;
}
.menu-row:hover { background: rgba(var(--accent), .12); color: #12212e; }
.menu-row:active {
  background: #fff;
  box-shadow: 0 0 5px rgba(var(--accent), .30), 0 0 1px rgba(var(--accent), .30);
}
.menu-row:active::after {
  content: ""; position: absolute; inset: 0; padding: 1px; border-radius: 2px;
  background: rgb(var(--accent));
  -webkit-mask: linear-gradient(#000 0 0) content-box, linear-gradient(#000 0 0);
          mask: linear-gradient(#000 0 0) content-box, linear-gradient(#000 0 0);
  -webkit-mask-composite: xor; mask-composite: exclude; pointer-events: none;
}
.menu-row--danger { color: rgb(var(--danger)); }
.menu-row--danger:hover { background: rgba(var(--danger), .12); }
.menu-row--danger:active { box-shadow: 0 0 5px rgba(var(--danger), .28), 0 0 1px rgba(var(--danger), .28); }
.menu-row--danger:active::after { background: rgb(var(--danger)); }
.menu-kbd { font: 600 10px/1 var(--font-mono); letter-spacing: .06em; color: rgb(var(--ink-faint)); }
.menu-sep { height: 1px; margin: 6px 8px; background: rgba(var(--accent), .18); }
```

- [ ] **Step 3: Wire the stylesheets + fonts + base into `app.css`**

Replace lines 1-7 of `packages/web/src/app.css`:
```css
/* was: @import "@fontsource-variable/source-sans-3"; */
@import "@fontsource/ibm-plex-sans";
@import "@fontsource/ibm-plex-mono";
@import "@xyflow/svelte/dist/style.css";
@import "./atlas.css";
@import "./atlas-components.css";
@import "./components/canvas/canvas.css";

@tailwind base;
@tailwind components;
@tailwind utilities;

body { font-family: var(--font-ui); color: var(--ink); }
```
(Keep the `.keyhint` block below it unchanged. Do NOT remove the `@tailwind` directives.)

- [ ] **Step 4: Swap font deps in `package.json`**

In `packages/web/package.json` dependencies (line 15), replace
`"@fontsource-variable/source-sans-3": "^5.2.9",`
with
```json
    "@fontsource/ibm-plex-mono": "^5.1.0",
    "@fontsource/ibm-plex-sans": "^5.1.0",
```
(alphabetical order is fine; exact patch version may float — use whatever `pnpm add` resolves in Step 5.)

- [ ] **Step 5: Install deps**

Run: `pnpm --filter @waml/web add @fontsource/ibm-plex-sans @fontsource/ibm-plex-mono && pnpm --filter @waml/web remove @fontsource-variable/source-sans-3`
Expected: lockfile updates, no errors. (This also normalises the `package.json` versions from Step 4.)

- [ ] **Step 6: Update `types.ts` — mono NODE_FONT + hexToTriple**

Replace line 8 of `packages/web/src/components/canvas/nodes/types.ts`:
```ts
export const NODE_FONT = "'IBM Plex Mono', ui-monospace, 'SF Mono', 'Cascadia Code', Menlo, monospace";

/** Profile stereotype colors are hex (`#eab308`); Atlas needs an rgb TRIPLE so
 *  a node can self-theme via style="--accent:<r,g,b>". Accepts #RGB or #RRGGBB;
 *  anything else (or undefined) falls back to the default blue triple. */
export function hexToTriple(hex?: string): string {
  const DEFAULT = "20, 150, 220";
  if (!hex) return DEFAULT;
  let h = hex.trim().replace(/^#/, "");
  if (/^[0-9a-fA-F]{3}$/.test(h)) h = h.split("").map((c) => c + c).join("");
  if (!/^[0-9a-fA-F]{6}$/.test(h)) return DEFAULT;
  const n = parseInt(h, 16);
  return `${(n >> 16) & 255}, ${(n >> 8) & 255}, ${n & 255}`;
}
```

- [ ] **Step 7: Start the devserver (leave running the whole pass)**

Run in the background: `pnpm --filter @waml/web dev`
Expected: `VITE ... ready`, serving `http://localhost:5173`. Open it once in a normal Edge window; the app should load with IBM Plex Sans text (no Source Sans 3). Nodes are still Tailwind-styled — that is expected; no surface is converted yet.

- [ ] **Step 8: Gate**

Run: `pnpm --filter @waml/web check` — Expected: PASS (no type/svelte errors).
Run: `pnpm --filter @waml/web build` — Expected: PASS (vite build completes; the two new fontsource packages resolve).
(No Tailwind-free grep this task — no Svelte surface converted. Both new CSS files contain zero `class="..."` markup, so they are trivially clean.)

- [ ] **Step 9: Commit**

```bash
git add packages/web/src/atlas.css packages/web/src/atlas-components.css \
  packages/web/src/app.css packages/web/package.json pnpm-lock.yaml \
  packages/web/src/components/canvas/nodes/types.ts
git commit -m "feat(web): add Atlas token + material kit, swap to IBM Plex fonts"
```

---

## Task 2: Node leaves — StereotypeRow / AttributeRow / RowsCompartment

**Files:**
- Modify: `packages/web/src/components/canvas/nodes/StereotypeRow.svelte` (whole template)
- Modify: `packages/web/src/components/canvas/nodes/AttributeRow.svelte` (whole template)
- Modify: `packages/web/src/components/canvas/nodes/RowsCompartment.svelte` (the two `<div class="border-t …">` wrappers + the more/less button)

**Interfaces:**
- Consumes (from Task 1): `.node-stereo`, `.node-row`, `.node-row__vis`, `.node-row__name`, `.node-row__ty`, `.node-comp`, `.node-more`.
- Produces: leaves emit Atlas classes; ClassifierBox (Task 3) wraps them.

**Verify pairing:** rows/stereo section of `hud-node-mock.html` (the `.hdr`/`.rows` block).

- [ ] **Step 1: Convert `StereotypeRow.svelte`**

Replace the template (lines 5-10) with:
```svelte
{#if keyword || stereotypes.length > 0}
  <div class="node-stereo">
    {#if keyword}<span>{`«${keyword}»`}</span>{/if}
    {#each stereotypes as s (s)}<span>{`«${s}»`}</span>{/each}
  </div>
{/if}
```
(`.node-stereo` already renders accent mono uppercase and centers; guillemets pass through.)

- [ ] **Step 2: Convert `AttributeRow.svelte`**

Replace the template (lines 10-18) with:
```svelte
<div class="node-row">
  {#if showVisibility && a.visibility}<span class="node-row__vis">{a.visibility}</span>{/if}
  <span class="node-row__name" title={a.name}>{a.name}</span>
  {#if showTypes || showMultiplicity}
    <span class="node-row__ty">
      {#if showTypes}: {a.type.name}{/if}{#if showMultiplicity && a.multiplicity !== "1"} {`{${a.multiplicity}}`}{/if}
    </span>
  {/if}
</div>
```

- [ ] **Step 3: Convert `RowsCompartment.svelte`**

Replace both `<div class="border-t border-[#eef1f5]">` wrappers (lines 17 and 30) with `<div class="node-comp">`.
Replace the `+K more` static footer (lines 22-24) with:
```svelte
        <div class="node-more" style="cursor:default">
          +{hiddenK} more
        </div>
```
Replace the interactive more/less `<button ...>` (lines 35-43) with:
```svelte
        <button
          onclick={(e) => { e.stopPropagation(); expanded = !expanded; }}
          class="node-more"
        >
          {#if expanded}
            <ChevronDown size={12} /> Show less
          {:else}
            <ChevronRight size={12} /> +{hidden} more
          {/if}
        </button>
```

- [ ] **Step 4: Tailwind-free grep on all three files**

Run the two `rg` commands from the standing prerequisites against each of the three files. Expected: ZERO hits each.

- [ ] **Step 5: Visual verify**

Screenshot the running app (a node showing a stereotype + attribute rows — the rows will render with Atlas typography even though ClassifierBox is still Tailwind; the header wrapper lands in Task 3) and the `hud-node-mock.html` rows. `Read` both; confirm accent mono `«kw»`, `+`/`-` visibility markers in accent, type text in faint gray, monospace uppercase-ish rows.

- [ ] **Step 6: Gate**

Run: `pnpm --filter @waml/web check` — Expected: PASS.
Run: `pnpm --filter @waml/web build` — Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add packages/web/src/components/canvas/nodes/StereotypeRow.svelte \
  packages/web/src/components/canvas/nodes/AttributeRow.svelte \
  packages/web/src/components/canvas/nodes/RowsCompartment.svelte
git commit -m "feat(web): convert node leaf rows to Atlas classes"
```

---

## Task 3: ClassifierBox keystone + NodePorts + self-theming

**Files:**
- Modify: `packages/web/src/components/canvas/nodes/ClassifierBox.svelte` (script `boxStyle` + whole template)
- Modify: `packages/web/src/components/canvas/nodes/NodePorts.svelte` (the `dotBase` string)

**Interfaces:**
- Consumes: `hexToTriple` (Task 1, `./types`), `.hud-surface`, `.hud-surface--node`, `.hud-node`, `.hud-node__body`, `.node-hdr(--band|--fill)`, `.node-name(--abstract)`, `.node-summary`, `.node-port` (Task 1); `StereotypeRow`/`AttributeRow`/`RowsCompartment` (Task 2).
- Produces: a self-theming Atlas node card that `GenericNode`, `OkfNode`, `UmlClassNode`, `UmlInterfaceNode`, `UmlEnumNode`, `UmlDataTypeNode`, `UmlAssociationNode`, `UmlPackageNode` all render through unchanged.

**Verify pairing:** `hud-node-mock.html` + the live-preview card in `hud-node-design-mock.html`.

- [ ] **Step 1: Rewrite the `boxStyle` derived (ClassifierBox script, lines 31-42)**

Replace lines 31-42 with:
```ts
  let headerColor = $derived(overrideHeader ?? st.header);
  let accentTriple = $derived(hexToTriple(headerColor));

  // Structural per-node style: self-theme the accent triple; thick border and
  // hexagon shape stay as inline structural declarations (not Tailwind).
  let boxStyle = $derived.by(() => {
    const decls: string[] = [`--accent:${accentTriple}`];
    if (st.border === "thick") decls.push(`--bw:2.5px`);
    if (st.shape === "hexagon") {
      decls.push(`clip-path:polygon(8% 0, 92% 0, 100% 50%, 92% 100%, 8% 100%, 0 50%)`);
    }
    return decls.join(";");
  });
```
Also delete the now-unused `NODE_FONT` import on line 9 (keep the `type OkfNodeData` import) and add `hexToTriple`:
```ts
  import { hexToTriple, type OkfNodeData } from "./types";
```

- [ ] **Step 2: Rewrite the ClassifierBox template (lines 45-81)**

```svelte
<div
  data-stereotyped={hasStereotypeStyle ? true : undefined}
  class="hud-surface hud-surface--node hud-node"
  style={boxStyle}
>
  <NodePorts />
  <div class="hud-node__body">
    {@render header?.()}
    <div class={`node-hdr ${headerColor ? "node-hdr--fill" : "node-hdr--band"}`}>
      {#if showStereotype}
        <StereotypeRow stereotypes={stereotypeTags} {keyword} />
      {/if}
      <div class={`node-name ${data.abstract ? "node-name--abstract" : ""}`}>
        {data.concept.title ?? "Untitled"}
      </div>
    </div>
    {#if isDetailed && data.values && data.values.length > 0}
      <RowsCompartment rows={data.values.length}>
        {#snippet render(i: number)}
          <div class="node-row"><span class="node-row__name">{data.values?.[i]}</span></div>
        {/snippet}
      </RowsCompartment>
    {/if}
    {#if isDetailed && !data.values}
      <RowsCompartment rows={data.attributes.length} max={display.maxAttributes}>
        {#snippet render(i: number)}
          <AttributeRow a={data.attributes[i]} {showVisibility} {showTypes} showMultiplicity={display.showAttributeMultiplicity} />
        {/snippet}
      </RowsCompartment>
    {/if}
    {#if !isDetailed}
      <div class="node-summary">
        {data.values ? `${data.values.length} values` : `${data.attributes.length} attribute${data.attributes.length === 1 ? "" : "s"}`}
      </div>
    {/if}
  </div>
</div>
```
Note: the `.node-name` moved INSIDE `.node-hdr` (mock groups stereotype + name in one bordered header). The old `w-[230px]`/`cursor-grab`/`select-none`/hover-border and the `bg-white border rounded-xl shadow` all disappear — `.hud-node` + `.hud-surface--node` supply width, grab cursor, frost, frame, glow.

- [ ] **Step 3: Convert `NodePorts.svelte`**

Replace the `dotBase` string (lines 17-19) and the four `<Handle>` lines so the dots use the `.node-port` class instead of an inline color literal. The class already carries size/shape/accent/opacity/transform; only per-side offsets stay inline:
```svelte
  // Per-side offsets only; visual style comes from the global `.node-port` class
  // (accent-themed, revealed on hover/select via canvas.css .node-handle rule).
```
```svelte
<Handle type="source" position={Position.Left} id="l" isConnectable class="node-handle node-port" style="left:0;top:50%;" />
<Handle type="source" position={Position.Right} id="r" isConnectable class="node-handle node-port" style="left:100%;top:50%;" />
<Handle type="source" position={Position.Top} id="t" isConnectable class="node-handle node-port" style="left:50%;top:0;" />
<Handle type="source" position={Position.Bottom} id="b" isConnectable class="node-handle node-port" style="left:50%;top:100%;" />
```
(Delete the `dotBase` const entirely. `.node-handle` in canvas.css still drives the hover/select opacity reveal.)

- [ ] **Step 4: Tailwind-free grep**

Run the two `rg` commands against `ClassifierBox.svelte` and `NodePorts.svelte`. Expected: ZERO hits each.

- [ ] **Step 5: Visual verify (+ self-theming)**

Screenshot the running app with (a) a plain node and (b) a node whose stereotype maps to a color (e.g. `«aggregateRoot»` → gold `#eab308`, `«valueObject»` → slate `#64748b` from `umlDomain.ts`). Confirm the plain node is blue-framed and the colored one re-themes its whole frame/glow/header to the stereotype hue (proving `hexToTriple` + inline `--accent`). Compare against `hud-node-mock.html` (blue) and the design-mock card. Verify Plex Mono uppercase identifiers.

- [ ] **Step 6: Gate**

Run: `pnpm --filter @waml/web check` — Expected: PASS (watch for the removed `NODE_FONT` import — must be gone).
Run: `pnpm --filter @waml/web build` — Expected: PASS.
Also run the existing component test to confirm no structural break: `pnpm --filter @waml/web test -- ClassifierBox` — Expected: PASS or update assertions that referenced old Tailwind classes (adjust the test to the new class names if it asserts on them).

- [ ] **Step 7: Commit**

```bash
git add packages/web/src/components/canvas/nodes/ClassifierBox.svelte \
  packages/web/src/components/canvas/nodes/NodePorts.svelte \
  packages/web/src/components/canvas/nodes/ClassifierBox.test.ts
git commit -m "feat(web): Atlas ClassifierBox keystone + self-theming ports"
```

---

## Task 4: GenericNode badge

**Files:**
- Modify: `packages/web/src/components/canvas/nodes/GenericNode.svelte` (the header snippet)

**Interfaces:**
- Consumes: `.node-badge` (Task 1); `ClassifierBox` header snippet slot (Task 3).

**Verify pairing:** `hud-node-mock.html` (badge reads as a square accent-wash chip, not a gray pill).

- [ ] **Step 1: Convert the badge snippet (GenericNode lines 9-15)**

```svelte
  {#snippet header()}
    <span class="node-badge">{data.type}</span>
  {/snippet}
```
(Drops `rounded-full bg-[#94a3b8] text-white` + the wrapper padding div; `.node-badge` supplies square corners, accent wash, mono uppercase, and its own top/side margin.)

- [ ] **Step 2: Tailwind-free grep**

Run the two `rg` commands against `GenericNode.svelte`. Expected: ZERO hits.

- [ ] **Step 3: Visual verify**

Screenshot a GenericNode (non-UML type) in the app; confirm a square accent-tinted uppercase badge at the top-left of the card. Compare against the mock chip treatment.

- [ ] **Step 4: Gate**

Run: `pnpm --filter @waml/web check` — Expected: PASS.
Run: `pnpm --filter @waml/web build` — Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add packages/web/src/components/canvas/nodes/GenericNode.svelte
git commit -m "feat(web): Atlas GenericNode square badge chip"
```

---

## Task 5: Edges — RelEdge / AnchorEdge + canvas.css selection

**Files:**
- Modify: `packages/web/src/components/canvas/RelEdge.svelte` (script `stroke`/`edgeStyle`, marker fills, EdgeLabel styles)
- Modify: `packages/web/src/components/canvas/AnchorEdge.svelte` (the one `<BaseEdge>` style)
- Modify: `packages/web/src/components/canvas/canvas.css` (selected-node ring, selected-edge stroke)

**Interfaces:**
- Consumes: tokens `--accent`, `--ink-faint` (triples) resolving on the SVG elements (decision C).
- Produces: edges + selection styled off tokens; no `#1e88e5`/`#64748b`/`#94a3b8`/`#e6f1fb` literals remain in these three files.

**Verify pairing:** `hud-edges-mock.html`.

- [ ] **Step 1: RelEdge stroke + edgeStyle (lines 78-82)**

```ts
  const stroke = $derived(selected ? "rgb(var(--accent))" : "rgb(var(--ink-faint))");
  const strokeWidth = $derived(selected ? 2.5 : 1.8);
  const edgeStyle = $derived(
    `stroke:${stroke};stroke-width:${strokeWidth};${DASHED.has(kind) ? "stroke-dasharray:6 4;" : ""}${
      selected ? "filter:drop-shadow(0 0 2.5px rgba(var(--accent),.35));" : ""
    }${style ?? ""}`,
  );
```
(`stroke` is reused in the marker `fill`/`stroke` attributes below — since it is now `rgb(var(--accent))`/`rgb(var(--ink-faint))`, the markers re-theme with the edge automatically. The composition filled-diamond `fill={m.fill}` where `m.fill = stroke` still works.)

- [ ] **Step 2: RelEdge marker hollow fills (lines 155-173)**

Change the two `fill="#fff"` literals (hollow triangle line 160, and the hollow-diamond case which already uses `m.fill` = `"#fff"` from line 99) to the panel fill so hollow heads read as outlines on the frosted canvas. In the marker `<path>` for `triangle`, replace `fill="#fff"` with `fill="var(--panel-fill)"`. In the script (line 99) replace `fill: "#fff"` with `fill: "var(--panel-fill)"`.

- [ ] **Step 3: RelEdge label styles (lines 190, 200, 210) — frost + mono**

Replace each of the three `EdgeLabel` `style="background:rgba(255,255,255,0.9);…color:#334155;…"` strings with a frost + mono treatment:
```
style="background:linear-gradient(180deg,rgba(255,255,255,.95),rgba(255,255,255,.82));border-radius:0;padding:1px 5px;font-family:var(--font-mono);font-size:10.5px;font-weight:600;letter-spacing:.04em;color:var(--ink);box-shadow:0 0 0 1px rgba(var(--accent),.22);white-space:nowrap;"
```
(Apply the identical string to all three labels — role/cardinality label, name label, stereo label.)

- [ ] **Step 4: AnchorEdge stroke (line 29)**

```svelte
  <BaseEdge {id} {path} style="stroke:rgb(var(--ink-faint));stroke-width:1.2;stroke-dasharray:4 3;" />
```

- [ ] **Step 5: canvas.css selection (lines 19-29)**

Replace the selected-node ring and selected-edge blocks:
```css
/* Node selected: brighten + thicken the existing masked accent frame and bloom.
   No border-color literal — the frame is the .hud-surface ::before, driven by --bw. */
.svelte-flow__node.selected > div {
  --bw: 2.5px;
  box-shadow:
    0 8px 22px rgba(40, 70, 110, 0.14),
    0 0 26px rgba(var(--accent), 0.28) !important;
}

/* Edge selected */
.svelte-flow__edge.selected .svelte-flow__edge-path {
  stroke: rgb(var(--accent));
  stroke-width: 2.5;
}
```
(Also update the `.node-handle` border literal if present — it now comes from `.node-port`; the `opacity:1 !important` rule at the top of canvas.css stays.)

- [ ] **Step 6: Tailwind-free grep + literal scan**

Run the two `rg` commands against `RelEdge.svelte` and `AnchorEdge.svelte` (they use inline SVG strings, not `class="…"` Tailwind, so expect ZERO hits). Additionally confirm no stale color literals:
```
rg -n '#1e88e5|#64748b|#94a3b8|#e6f1fb|#334155' packages/web/src/components/canvas/RelEdge.svelte packages/web/src/components/canvas/AnchorEdge.svelte packages/web/src/components/canvas/canvas.css
```
Expected: ZERO hits.

- [ ] **Step 7: Visual verify**

Screenshot the app with an edge selected and a couple of relationship kinds (association arrow, generalization hollow triangle, composition filled diamond) visible; compare against `hud-edges-mock.html`. Confirm faint-gray resting strokes, accent selected stroke + subtle glow, hollow heads reading as outlines, frost mono labels.

- [ ] **Step 8: Gate**

Run: `pnpm --filter @waml/web check` — Expected: PASS.
Run: `pnpm --filter @waml/web build` — Expected: PASS.

- [ ] **Step 9: Commit**

```bash
git add packages/web/src/components/canvas/RelEdge.svelte \
  packages/web/src/components/canvas/AnchorEdge.svelte \
  packages/web/src/components/canvas/canvas.css
git commit -m "feat(web): Atlas edges + token-driven selection ring"
```

---

## Task 6: InspectorPanel + ElementPicker

**Files:**
- Modify: `packages/web/src/components/inspector/InspectorPanel.svelte` (shell `<aside>` class, resize handle, kind badge, header buttons)
- Modify: `packages/web/src/components/inspector/ElementPicker.svelte` (trigger button, listbox, option rows)

**Interfaces:**
- Consumes: `.hud-surface` (panel default knobs), token vars.
- Produces: inspector shell + picker fully off Tailwind. (Section headers / member rows shown in the mock are not rendered by these two files — they come from the inspector body children, which are out of this plan's ledger; convert only what these two files own.)

**Verify pairing:** `hud-inspector-mock.html`.

- [ ] **Step 1: Convert the `<aside>` shell (InspectorPanel lines 121-130)**

Keep the `style={`width: ${width}px`}` and the pointer/focus handlers; swap the class list. The translucency stays a conditional inline opacity so it composes with `.hud-surface`:
```svelte
<aside
  aria-label="Inspector"
  style={`width: ${width}px; ${translucent ? "opacity:.4" : "opacity:1"}`}
  class="hud-surface insp-panel"
  onpointerenter={engage}
  onpointerleave={disengage}
  onfocusin={engage}
  onfocusout={disengage}
>
```
Add a scoped `<style>` block at the end of the component for inspector-only structure (position, sizing, overflow, transition — not Tailwind, so allowed as scoped CSS):
```svelte
<style>
  .insp-panel {
    position: absolute; top: 12px; right: 12px;
    max-width: calc(100% - 24px); max-height: calc(100% - 24px);
    overflow: hidden; z-index: 16;
    display: flex; flex-direction: column;
    transition: opacity .2s ease;
  }
  .insp-resize { position: absolute; left: 0; top: 0; bottom: 0; width: 6px; margin-left: -3px; cursor: col-resize; z-index: 17; }
  .insp-resize:hover { background: rgba(var(--accent), .20); }
  .insp-head { display: flex; align-items: center; gap: 8px; padding: 12px; position: relative; z-index: 1; }
  .insp-head--divide { border-bottom: 1px solid rgba(var(--accent), .22); }
  .insp-kind {
    flex: none; width: 26px; height: 26px; display: grid; place-items: center;
    border-radius: 2px; color: rgb(var(--accent)); background: rgba(var(--accent), .12);
  }
  .insp-iconbtn {
    width: 30px; height: 30px; display: grid; place-items: center; border: 0; background: transparent;
    border-radius: 2px; color: rgb(var(--ink-faint)); cursor: pointer;
  }
  .insp-iconbtn:hover { background: rgba(var(--accent), .12); color: rgb(var(--accent)); }
  .insp-iconbtn.is-active { color: rgb(var(--accent)); background: rgba(var(--accent), .12); }
  .insp-body { flex: 1; min-height: 0; overflow-y: auto; position: relative; z-index: 1; }
  .insp-body-inner { padding: 16px; }
</style>
```

- [ ] **Step 2: Convert the resize handle (lines 132-139)**

```svelte
  {#if hasSelection && !collapsed}
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div onmousedown={onResizeMouseDown} title="Drag to resize" class="insp-resize"></div>
  {/if}
```

- [ ] **Step 3: Convert the header row + kind badge + buttons (lines 141-187)**

```svelte
  <div class={`insp-head ${hasSelection && !collapsed ? "insp-head--divide" : ""}`}>
    {#if focusedKind}
      {@const KindIcon = KIND_ICON[focusedKind]}
      <span class="insp-kind"><KindIcon size={15} /></span>
    {/if}
    <div style="flex:1;min-width:0"><ElementPicker {options} {selectedKey} {onSelect} /></div>
    {#if hasSelection}
      <button onclick={onEdit} aria-label="Edit element" title="Edit element" class="insp-iconbtn"><Pencil size={15} /></button>
    {/if}
    {#if hasSelection}
      <button
        onclick={() => (collapsed = !collapsed)}
        aria-label={collapsed ? "Expand inspector" : "Collapse inspector"}
        aria-expanded={!collapsed}
        title={collapsed ? "Expand inspector" : "Collapse inspector"}
        class="insp-iconbtn"
      >
        <span class="insp-caret" style={collapsed ? "transform:rotate(180deg)" : ""}><ChevronUp size={16} /></span>
      </button>
    {/if}
    <button
      onclick={onTogglePin}
      aria-label={pinned ? "Let it dim when idle" : "Keep solid"}
      aria-pressed={pinned}
      title={pinned ? "Let it dim when idle" : "Keep solid"}
      class={`insp-iconbtn ${pinned ? "is-active" : ""}`}
    >
      {#if pinned}<Pin size={16} />{:else}<PinOff size={16} />{/if}
    </button>
  </div>
```
Add `.insp-caret { display: flex; transition: transform .2s ease; }` to the scoped `<style>`.

- [ ] **Step 4: Convert the body wrapper (lines 189-195)**

```svelte
  {#if hasSelection && !collapsed}
    <div class="insp-body">
      <div transition:foldFade={{ duration: 200 }} class="insp-body-inner">
        {@render children?.()}
      </div>
    </div>
  {/if}
```

- [ ] **Step 5: Convert `ElementPicker.svelte`**

Replace the trigger `<button>` class (line 97) and the two label spans (lines 99-107), the listbox container (line 120) and option rows (lines 133-144) with scoped classes. Add a scoped `<style>` to the component:
```svelte
<style>
  .ep-trigger {
    width: 100%; display: flex; align-items: center; gap: 6px; min-width: 0;
    font: 500 14px/1 var(--font-ui); border: 0; background: transparent;
    border-radius: 2px; padding: 4px 6px; cursor: pointer;
  }
  .ep-trigger:hover { background: rgba(var(--accent), .10); }
  .ep-label { flex: 1; text-align: left; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; color: var(--ink); font-weight: 600; }
  .ep-label--empty { color: rgb(var(--ink-faint)); font-weight: 500; }
  .ep-chev { flex: none; color: rgb(var(--ink-faint)); transition: transform .15s ease; }
  .ep-menu {
    position: fixed; z-index: 60; max-height: 280px; overflow-y: auto; padding: 6px;
    background: linear-gradient(180deg, rgba(255,255,255,.95), rgba(255,255,255,.82)), rgba(var(--accent), .06);
    box-shadow: 0 12px 30px rgba(40,70,110,.20), 0 0 calc(14px * var(--glow)) rgba(var(--accent), calc(.16 * var(--glow)));
  }
  .ep-empty { padding: 8px 12px; font: 500 13px/1 var(--font-ui); color: rgb(var(--ink-faint)); }
  .ep-opt {
    width: 100%; text-align: left; display: flex; align-items: center; gap: 7px;
    padding: 8px 11px; border: 0; background: transparent; border-radius: 2px; cursor: pointer;
    font: 500 13px/1 var(--font-ui); color: var(--ink);
  }
  .ep-opt.is-hi { background: rgba(var(--accent), .12); }
  .ep-opt.is-sel { color: rgb(var(--accent)); font-weight: 600; }
</style>
```
Trigger:
```svelte
<button
  bind:this={triggerEl}
  type="button" role="combobox" aria-label="Select element"
  aria-haspopup="listbox" aria-controls="inspector-element-listbox" aria-expanded={open}
  onclick={toggleMenu} onkeydown={onTriggerKeydown}
  class="ep-trigger"
>
  <span class={`ep-label ${selectedLabel ? "" : "ep-label--empty"}`}>{selectedLabel ?? placeholder}</span>
  <ChevronDown size={15} class="ep-chev" style={open ? "transform:rotate(180deg)" : ""} />
</button>
```
Listbox + options (the `.fixed.inset-0` scrim overlay keeps its behaviour; give it a plain class):
```svelte
{#if open}
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="ep-scrim" onclick={closeMenu}></div>
  <div id="inspector-element-listbox" role="listbox" aria-label="Select element" tabindex="-1" style={menuStyle} class="ep-menu">
    {#if options.length === 0}
      <div class="ep-empty">No elements in this diagram</div>
    {/if}
    {#each options as opt, i (opt.key)}
      {@const RowIcon = KIND_ICON[opt.kind]}
      <button
        type="button" role="option" aria-selected={opt.key === selectedKey}
        onclick={() => choose(opt.key, opt.kind)} onmouseenter={() => (highlighted = i)}
        class={`ep-opt ${i === highlighted ? "is-hi" : ""} ${opt.key === selectedKey ? "is-sel" : ""}`}
      >
        <RowIcon size={14} style="flex:none" />
        <span style="overflow:hidden;text-overflow:ellipsis;white-space:nowrap">{opt.label}</span>
        {#if opt.key === selectedKey}<Check size={14} style="margin-left:auto;flex:none" />{/if}
      </button>
    {/each}
  </div>
{/if}
```
Add to the scoped `<style>`: `.ep-scrim { position: fixed; inset: 0; z-index: 59; }`.

- [ ] **Step 6: Tailwind-free grep + literal scan**

Run the two `rg` commands against `InspectorPanel.svelte` and `ElementPicker.svelte`. Expected: ZERO hits. Then:
```
rg -n '#1e88e5|#e6f1fb|#d8dee8|#f1f3f7' packages/web/src/components/inspector/InspectorPanel.svelte packages/web/src/components/inspector/ElementPicker.svelte
```
Expected: ZERO hits.

- [ ] **Step 7: Visual verify**

Open the inspector in the app (select an element). Screenshot; compare against `hud-inspector-mock.html`: frosted panel, thin accent frame, low glow, accent kind badge, hover-accent icon buttons, picker option rows with accent selected state.

- [ ] **Step 8: Gate**

Run: `pnpm --filter @waml/web check` — Expected: PASS.
Run: `pnpm --filter @waml/web build` — Expected: PASS.
Run: `pnpm --filter @waml/web test -- InspectorPanel` — Expected: PASS (or update any assertions on old class names).

- [ ] **Step 9: Commit**

```bash
git add packages/web/src/components/inspector/InspectorPanel.svelte \
  packages/web/src/components/inspector/ElementPicker.svelte \
  packages/web/src/components/inspector/InspectorPanel.test.ts
git commit -m "feat(web): Atlas inspector panel + element picker"
```

---

## Task 7: SelectionToolbar (+ press-ripple action)

**Files:**
- Create: `packages/web/src/lib/hudPress.ts` (Svelte `use:` action for the button ripple)
- Modify: `packages/web/src/components/canvas/SelectionToolbar.svelte` (whole toolbar markup)

**Interfaces:**
- Consumes: `.hud-surface`, `.hud-btn`, `.hud-btn--sm`, `.hud-ctrl`, `.menu-kbd`, tokens.
- Produces: `hudPress` action; the toolbar pill + buttons on Atlas material. Danger delete button re-themes by overriding `--accent` to the danger triple inline.

**Verify pairing:** `hud-button-mock.html` (press ripple) + `hud-menu-mock.html` (danger row hue).

- [ ] **Step 1: Create `hudPress.ts`**

```ts
// Press-ripple for .hud-btn: on pointerdown set the reveal origin (--ox/--oy)
// from the click point and add `.down` so the ::after frame reveals and the
// glowpulse fires; remove it on release. CSS lives in atlas-components.css.
export function hudPress(node: HTMLElement) {
  function down(e: PointerEvent) {
    const r = node.getBoundingClientRect();
    node.style.setProperty("--ox", `${(((e.clientX - r.left) / r.width) * 100).toFixed(1)}%`);
    node.style.setProperty("--oy", `${(((e.clientY - r.top) / r.height) * 100).toFixed(1)}%`);
    node.classList.remove("down");
    void node.offsetWidth; // reflow to restart the animation
    node.classList.add("down");
  }
  function up() {
    node.classList.remove("down");
  }
  node.addEventListener("pointerdown", down);
  node.addEventListener("pointerup", up);
  node.addEventListener("pointercancel", up);
  return {
    destroy() {
      node.removeEventListener("pointerdown", down);
      node.removeEventListener("pointerup", up);
      node.removeEventListener("pointercancel", up);
    },
  };
}
```

- [ ] **Step 2: Import the action + drop the Source Sans font literal (SelectionToolbar script)**

Add to the `<script>` imports:
```ts
  import { hudPress } from "../../lib/hudPress";
```

- [ ] **Step 3: Convert the toolbar markup (lines 62-122)**

```svelte
<div
  data-testid="selection-toolbar"
  class="nopan nodrag st-dock"
  transition:fly={{ y: 12, duration: 150 }}
>
  <div class="hud-surface st-pill">
    {#if naming}
      <!-- svelte-ignore a11y_autofocus -->
      <input
        aria-label="New diagram name"
        bind:value={name}
        onkeydown={onKey}
        placeholder="New diagram name"
        autofocus
        class="st-input"
      />
      <button use:hudPress onclick={confirm} aria-label="Create diagram" class="hud-surface hud-surface--btn hud-btn hud-btn--sm">Create diagram</button>
      <button onclick={cancel} aria-label="Cancel" class="st-text">Cancel</button>
    {:else}
      <span class="st-summary">{summary}</span>
      <div class="st-sep"></div>
      <button
        use:hudPress
        onclick={startNaming}
        disabled={!canCreate}
        aria-label="New diagram from selection"
        title={canCreate ? "New diagram seeded with the selected objects" : "Select at least one object to create a diagram"}
        class="hud-surface hud-surface--btn hud-btn hud-btn--sm st-action"
      >
        <LayoutDashboard size={14} /> New diagram from selection
      </button>
      <button
        use:hudPress
        onclick={onDelete}
        aria-label="Delete selection"
        title="Delete the selected objects and relationships"
        class="hud-surface hud-surface--btn hud-btn hud-btn--sm st-action st-danger"
        style="--accent:235, 70, 120"
      >
        <Trash2 size={14} /> Delete selection
        <KeyHint keys={keyLabel("selection.delete")} />
      </button>
    {/if}
  </div>
</div>

<style>
  .st-dock { position: fixed; bottom: 24px; left: 50%; transform: translateX(-50%); z-index: 30; }
  .st-pill { display: flex; align-items: center; gap: 6px; padding: 6px; }
  .st-summary { padding: 0 8px; font: 500 12px/1 var(--font-ui); color: rgb(var(--ink-faint)); white-space: nowrap; }
  .st-sep { width: 1px; height: 20px; background: rgba(var(--accent), .18); }
  /* compact hud-btn overrides for the toolbar: real text label, not wide caps */
  .st-action, .st-danger { display: inline-flex; align-items: center; gap: 6px; text-transform: none; letter-spacing: .02em; font-weight: 600; font-family: var(--font-ui); font-size: 12px; }
  .st-input {
    width: 180px; font: 500 13px/1 var(--font-ui); padding: 6px 8px; color: var(--ink);
    background: #fff; border: 1px solid rgba(var(--accent), .26); border-radius: 3px; outline: 0;
  }
  .st-input:focus { border-color: rgb(var(--accent)); box-shadow: 0 0 0 1px rgb(var(--accent)); }
  .st-text { border: 0; background: transparent; cursor: pointer; padding: 7px 10px; border-radius: 2px; font: 500 12px/1 var(--font-ui); color: rgb(var(--ink-faint)); }
  .st-text:hover { background: rgba(var(--accent), .12); color: rgb(var(--accent)); }
</style>
```
Notes: the `.st-action`/`.st-danger` scoped rules override `.hud-btn`'s wide uppercase mono label back to a normal-case UI label (toolbar buttons are small, not the hero button). The delete button re-themes to danger purely by the inline `--accent:235, 70, 120`, so its frame + glow + ripple all go rose — no separate danger CSS needed. The old `style="font-family: 'Source Sans 3 Variable' …"` on the outer div is removed (font now inherits `--font-ui`).

- [ ] **Step 4: Tailwind-free grep + literal scan**

Run the two `rg` commands against `SelectionToolbar.svelte`. Expected: ZERO hits. Then:
```
rg -n '#1e88e5|#dc2626|#d8dee8|Source Sans' packages/web/src/components/canvas/SelectionToolbar.svelte
```
Expected: ZERO hits.

- [ ] **Step 5: Visual verify**

In the app, select 1+ nodes to mount the toolbar. Screenshot; confirm the frosted pill, the two Atlas action buttons, and (mousing down) the accent ripple. Confirm the Delete button frame/glow is rose (danger triple) and its press ripple is rose. Compare against `hud-button-mock.html` (ripple) and `hud-menu-mock.html` (danger hue).

- [ ] **Step 6: Gate**

Run: `pnpm --filter @waml/web check` — Expected: PASS.
Run: `pnpm --filter @waml/web build` — Expected: PASS.
Run: `pnpm --filter @waml/web test -- SelectionToolbar` — Expected: PASS (or update assertions on old class names).

- [ ] **Step 7: Commit**

```bash
git add packages/web/src/lib/hudPress.ts \
  packages/web/src/components/canvas/SelectionToolbar.svelte \
  packages/web/src/components/canvas/SelectionToolbar.test.ts
git commit -m "feat(web): Atlas selection toolbar + hud press-ripple action"
```

---

## Task 8: Non-classifier UML nodes (note / actor / use-case / package)

**Files:**
- Modify: `packages/web/src/components/canvas/nodes/UmlUseCaseNode.svelte` (whole template + scoped `<style>`)
- Modify: `packages/web/src/components/canvas/nodes/UmlNoteNode.svelte` (whole template + scoped `<style>`)
- Modify: `packages/web/src/components/canvas/nodes/UmlActorNode.svelte` (whole template + scoped `<style>`)
- Modify: `packages/web/src/components/canvas/nodes/UmlPackageNode.svelte` (the tab wrapper only — the box body rides the Task-3 ClassifierBox unchanged)

**Interfaces:**
- Consumes: `hexToTriple` (Task 1, `./types`) for the inline `--accent` self-theme (default blue triple — these shapes carry no stereotype color); the frost recipe + tokens (`--accent`, `--bw`, `--round`, `--glow`, `--panel-fill`, `--ink`, `--ink-dim`, `--font-mono`) from Tasks 1/3; `NodePorts` (Task 3, already Atlas) for use-case / note / actor; `ClassifierBox` (Task 3) for the package body.
- Produces: the four non-classifier shapes on Atlas material — each keeps its own silhouette (ellipse / folded-corner / stickman / tabbed folder) but wears the frost fill + accent frame-or-stroke + low glow + self-themed `--accent`; all four fully off Tailwind + hex.

**Shape/frame note (read before Step 1):** only the **ellipse** (use-case) can ride `.hud-surface` directly — its `border-radius:50%` is inherited by the masked `::before` frame and by the `box-shadow` glow, so both wrap the ellipse. The **folded-corner note** and the **stickman actor** cannot: `clip-path` clips away the `box-shadow` glow and the `::before`'s `inset:0` rectangle ignores the fold, so they get a bespoke token recipe — a two-layer clip-path frame + `drop-shadow` glow for the note, and stroke-recolored SVG + `drop-shadow` glow for the actor (a glyph, not a panel). The **package** reuses the already-Atlas ClassifierBox for its body and only converts its little tab.

**Verify pairing:** `hud-icons-mock.html` (the Package / Note / stickman glyph treatments — accent strokes, hollow frost interiors, source-bright frame) + `hud-node-mock.html` (the shared frost / frame / glow material tokens the four must match). There is no dedicated full-node mock for these shapes; pair the shape against the icons mock and the material against the node mock.

- [ ] **Step 1: Convert `UmlUseCaseNode.svelte` (ellipse — rides `.hud-surface`)**

Swap the import (line 3) to pull `hexToTriple`:
```svelte
  import { hexToTriple, type OkfNodeData } from "./types";
```
Replace the template (lines 8-17) with the ellipse riding the node material; the masked frame + glow follow `border-radius:50%` (inherited):
```svelte
<!-- UML UseCase: ellipse with the name centered, on Atlas node material. -->
<div class="hud-surface hud-surface--node uc-node" style={`--accent:${hexToTriple()}`}>
  <NodePorts />
  <div class="uc-name">{data.concept.title ?? data.key}</div>
</div>

<style>
  .uc-node {
    width: 190px; height: 72px; border-radius: 50%;
    display: flex; align-items: center; justify-content: center;
    padding: 0 20px; cursor: grab; user-select: none;
  }
  .uc-name {
    position: relative; z-index: 1; text-align: center;
    font: 700 12.5px/1.2 var(--font-mono);
    letter-spacing: .06em; text-transform: uppercase; color: var(--ink);
  }
</style>
```
(The scoped `.uc-node` border-radius wins over `.hud-surface`'s `var(--round)` via Svelte's added scope class; `.hud-surface::before` uses `border-radius:inherit`, so the accent frame + glow are elliptical. Drops `border-[#c8d2e0] bg-white shadow-[…] text-slate-800` + all Tailwind.)

- [ ] **Step 2: Convert `UmlNoteNode.svelte` (folded-corner — bespoke two-layer frame)**

Swap the import (line 3):
```svelte
  import { hexToTriple, type OkfNodeData } from "./types";
```
Replace the template (lines 11-20) with the two-layer clip-path recipe. The root is the source-bright accent gradient (the frame); a `::before` inset by `--bw` carries the frost fill in the same folded silhouette; the glow is a `drop-shadow` filter (follows the clip-path alpha, unlike `box-shadow`); a `::after` fills the dog-ear crease:
```svelte
<!-- UML Comment: a dog-eared note carrying the markdown body. clip-path fights
     .hud-surface's masked ::before frame + box-shadow glow, so this uses a bespoke
     two-layer clip-path frame + drop-shadow glow instead. NO attribute/operation
     compartments; dashed anchors are drawn by the edge/anchor layer. -->
<div class="note-node" style={`--accent:${hexToTriple()}`}>
  <NodePorts />
  <div class="note-body">
    {data.note_body ?? data.concept.title}
  </div>
</div>

<style>
  .note-node {
    --fold: 14px;
    position: relative; width: 210px; cursor: grab; user-select: none;
    /* accent layer = the source-bright frame, shows through the inner inset */
    background: linear-gradient(150deg, rgba(var(--accent), .95), rgba(var(--accent), .5));
    clip-path: polygon(0 0, calc(100% - var(--fold)) 0, 100% var(--fold), 100% 100%, 0 100%);
    filter:
      drop-shadow(0 6px 12px rgba(40, 70, 110, .14))
      drop-shadow(0 0 calc(10px * var(--glow)) rgba(var(--accent), calc(.5 * var(--glow))));
  }
  /* inner frost sheet, inset by the border weight, same folded silhouette */
  .note-node::before {
    content: ""; position: absolute; inset: var(--bw);
    background:
      linear-gradient(180deg, rgba(255, 255, 255, .94), rgba(255, 255, 255, .80)),
      rgba(var(--accent), .06);
    clip-path: polygon(0 0, calc(100% - var(--fold)) 0, 100% var(--fold), 100% 100%, 0 100%);
    pointer-events: none;
  }
  /* the dog-ear crease */
  .note-node::after {
    content: ""; position: absolute; top: 0; right: 0; z-index: 1;
    width: var(--fold); height: var(--fold);
    background: rgba(var(--accent), .18);
    clip-path: polygon(0 0, 100% 100%, 0 100%);
    pointer-events: none;
  }
  .note-body {
    position: relative; z-index: 1; padding: 9px 12px;
    font: 500 11.5px/1.5 var(--font-mono); color: var(--ink-dim);
    white-space: pre-wrap;
  }
</style>
```
(Drops the cream `bg-[#fffdf3] border-[#e3d9a8]` + `bg-[#f3ebc0]` fold + `shadow-[…]` + `text-slate-700` — the note is now a blue-frosted Atlas surface; losing the cream tint is the intended off-hex conversion. `NodePorts` handles keep `z-index:10` from `.node-port`, above both pseudos.)

- [ ] **Step 3: Convert `UmlActorNode.svelte` (stickman glyph — recolor + glow, no frame)**

Swap the import (line 3):
```svelte
  import { hexToTriple, type OkfNodeData } from "./types";
```
Replace the template (lines 8-21). The actor is a glyph, not a panel — no frost/frame; recolor the strokes to `rgb(var(--accent))`, fill the head hollow with the panel fill, add a soft accent `drop-shadow` glow, and set the name in Plex Mono uppercase:
```svelte
<!-- UML Actor: stick figure with the name beneath, on Atlas accent strokes. -->
<div class="actor-node" style={`--accent:${hexToTriple()}`}>
  <NodePorts />
  <svg class="actor-glyph" width="48" height="72" viewBox="0 0 48 72">
    <circle cx="24" cy="10" r="8" />
    <line x1="24" y1="18" x2="24" y2="44" />
    <line x1="6" y1="28" x2="42" y2="28" />
    <line x1="24" y1="44" x2="8" y2="66" />
    <line x1="24" y1="44" x2="40" y2="66" />
  </svg>
  <div class="actor-name">{data.concept.title ?? data.key}</div>
</div>

<style>
  .actor-node {
    position: relative; display: flex; flex-direction: column; align-items: center;
    width: 120px; cursor: grab; user-select: none;
  }
  .actor-glyph {
    position: relative; z-index: 1;
    fill: none; stroke: rgb(var(--accent)); stroke-width: 2;
    stroke-linecap: round; stroke-linejoin: round;
    filter: drop-shadow(0 0 calc(6px * var(--glow)) rgba(var(--accent), calc(.3 * var(--glow))));
  }
  .actor-glyph circle { fill: var(--panel-fill); }
  .actor-name {
    position: relative; z-index: 1; margin-top: 4px; max-width: 100%; text-align: center;
    font: 700 12px/1.2 var(--font-mono);
    letter-spacing: .06em; text-transform: uppercase; color: var(--ink);
  }
</style>
```
(Drops the `#334155`/`#fff` SVG literals + `text-slate-800 font-semibold` + Tailwind flex classes. Strokes + head-fill + glow now all self-theme off `--accent`.)

- [ ] **Step 4: Convert `UmlPackageNode.svelte` (tabbed folder — tab only)**

The body already rides the Atlas ClassifierBox (Task 3); only the little tab is Tailwind. Swap the import (line 2 stays `ClassifierBox`; line 3 adds `hexToTriple`) and replace the template (lines 8-12):
```svelte
<script lang="ts">
  import ClassifierBox from "./ClassifierBox.svelte";
  import { hexToTriple, type OkfNodeData } from "./types";

  let { data }: { data: OkfNodeData } = $props();
</script>

<!-- Tabbed-folder: a small frosted tab above the ClassifierBox body. -->
<div class="pkg-wrap" style={`--accent:${hexToTriple()}`}>
  <div class="pkg-tab"></div>
  <ClassifierBox {data} />
</div>

<style>
  .pkg-wrap { position: relative; }
  .pkg-tab {
    position: absolute; top: -10px; left: 10px; height: 12px; width: 64px;
    border-radius: var(--round) var(--round) 0 0;
    border: var(--bw) solid rgba(var(--accent), .5); border-bottom: 0;
    background:
      linear-gradient(180deg, rgba(255, 255, 255, .94), rgba(255, 255, 255, .80)),
      rgba(var(--accent), .06);
  }
</style>
```
(Drops `rounded-t-md border-[1.5px] border-b-0 border-[#d8dee8] bg-white`. The wrapper's inline `--accent` themes the tab; the ClassifierBox body computes its own `--accent` internally — for a plain package both are the default blue, so they match. CAVEAT: a package that ever carries a stereotype header color would theme its box body but leave the tab blue; not a case in the current data, flagged for a later pass.)

- [ ] **Step 5: Tailwind-free grep on all four files**

Run the two `rg` commands from the standing prerequisites against each of `UmlUseCaseNode.svelte`, `UmlNoteNode.svelte`, `UmlActorNode.svelte`, `UmlPackageNode.svelte`. Then confirm no stale hex literals:
```
rg -n '#fffdf3|#e3d9a8|#f3ebc0|#c8d2e0|#334155|#d8dee8|slate-' packages/web/src/components/canvas/nodes/UmlUseCaseNode.svelte packages/web/src/components/canvas/nodes/UmlNoteNode.svelte packages/web/src/components/canvas/nodes/UmlActorNode.svelte packages/web/src/components/canvas/nodes/UmlPackageNode.svelte
```
Expected: ZERO hits each.

- [ ] **Step 6: Visual verify**

Seed the app with a note, an actor, a use-case, and a package node (hand-drive the running devserver to a doc containing those element types, or use a sample doc). Screenshot; `Read` it against `hud-icons-mock.html` (glyph/shape treatment) + `hud-node-mock.html` (material). Confirm: use-case ellipse has the blue frost fill + elliptical accent frame + glow; the note is a blue-frosted folded sheet with an accent hairline that follows the fold diagonal + accent crease + drop-shadow glow; the actor stickman is accent-stroked with a soft glow + mono-uppercase name; the package tab is a small frosted accent-hairline tab continuous with the ClassifierBox frame.

- [ ] **Step 7: Gate**

Run: `pnpm --filter @waml/web check` — Expected: PASS.
Run: `pnpm --filter @waml/web build` — Expected: PASS.

- [ ] **Step 8: Commit**

```bash
git add packages/web/src/components/canvas/nodes/UmlUseCaseNode.svelte \
  packages/web/src/components/canvas/nodes/UmlNoteNode.svelte \
  packages/web/src/components/canvas/nodes/UmlActorNode.svelte \
  packages/web/src/components/canvas/nodes/UmlPackageNode.svelte
git commit -m "feat(web): Atlas non-classifier UML nodes (note/actor/use-case/package)"
```

---

## Tailwind-drop ledger (this pass)

Mark a surface done only when it is fully off Tailwind (grep clean) AND matches its mock.

| Surface | Task | Tailwind-free | Mock parity |
|---|---|---|---|
| foundation (atlas.css / components / fonts) | 1 | ☐ | — |
| StereotypeRow / AttributeRow / RowsCompartment | 2 | ☐ | ☐ |
| ClassifierBox (+ NodePorts) | 3 | ☐ | ☐ |
| GenericNode badge | 4 | ☐ | ☐ |
| RelEdge + AnchorEdge + canvas.css select | 5 | ☐ | ☐ |
| InspectorPanel + ElementPicker | 6 | ☐ | ☐ |
| SelectionToolbar | 7 | ☐ | ☐ |
| UmlUseCaseNode (ellipse) | 8 | ☐ | ☐ |
| UmlNoteNode (folded corner) | 8 | ☐ | ☐ |
| UmlActorNode (stickman) | 8 | ☐ | ☐ |
| UmlPackageNode (tabbed folder) | 8 | ☐ | ☐ |

Do NOT remove `tailwindcss`/`postcss` or the `@tailwind` directives — later-pass cleanup, gated on TopBar/Navigator/Dock/dialogs/sequence which this plan leaves untouched.

---

## Self-Review (performed against the spec before saving)

**Spec coverage:**
- S1 token layer → Task 1 (`atlas.css`, all listed tokens; `--ink-faint` promoted to a triple so `rgb(var(--ink-faint))` works in edge strings — a resolution of the spec's ambiguous "surfaces" grouping, noted inline).
- S2 material + all component classes (surface + knobs, head, button ripple, seg, toggle, chips, menu row, focus flare) → Task 1.
- 3.1 conversion order (8 items, one commit each, bottom-up) → Tasks 1-8 (items 1-7 exactly; Task 8 converts the four non-classifier UML shapes, added to 3.1 as item 8).
- 3.2 mapping table → Tasks 3 (ClassifierBox/header), 2 (rows/stereo), 4 (badge), 5 (RelEdge/canvas), 6 (inspector/kind badge), 7 (toolbar), 8 (UseCase/Note/Actor/Package shapes).
- 3.3 decision A (`hexToTriple`) → Task 1 (helper) + Task 3 (inline `--accent`). Decision B (Plex Mono uppercase identifiers + banked solver caveat) → Global Constraints + `.hud-node`/`.node-name` + NODE_FONT swap. Decision C (edge vars in style strings) → Task 5.
- 4.1 ledger + grep → included; 4.2 vite live-verify loop → standing prerequisites + every task's verify step; 4.3 out-of-scope guard → Global Constraints + File Structure notes.

**Placeholder scan:** no TBD / "handle edge cases" / "similar to Task N" — every code step shows full before/after content.

**Type consistency:** `hexToTriple(hex?: string): string` defined in Task 1, consumed in Task 3 with the same signature; `NODE_FONT` swapped in Task 1 and its sole consumer (ClassifierBox) updated in Task 3 (import dropped). Class names produced in Task 1 (`.hud-surface`, `.node-*`, `.hud-btn`, `.seg`, `.toggle`, `.menu-*`, `.hud-ctrl`) match every consumption in Tasks 2-7. `hudPress` action signature matches its `use:hudPress` call sites.

**Open questions / spec gaps flagged for the human:**
1. **Node `.selected` ripple origin.** The mock's click-origin `::after` reveal + `nodeglow` needs JS to set `--ox/--oy`; SvelteFlow only exposes a `.selected` class. This plan does the token-driven thicken-frame + bloom (spec 3.2 "accent masked frame + --glow bloom") and defers the true click-origin ripple to the shader/makepad port, matching the mock's own "waits for the shader port" note. Confirm that is acceptable for the web pass.
2. **`--ink-faint` as a triple.** Spec S1 lists it beside the hex inks but 3.2 uses `rgb(var(--ink-faint))`. Resolved as a triple. Flagging in case a hex form is wanted elsewhere.
3. **Compact `.hud-btn` in the toolbar.** The hero `.hud-btn` is wide uppercase mono; toolbar buttons are small text buttons. Task 7 keeps the Atlas frame/glow/ripple but overrides the label back to normal-case UI type via `.st-action`. Confirm the toolbar should reuse `.hud-btn` material (per spec 3.1 item 7) rather than a lighter treatment.
4. **`.node-hdr--fill` trigger.** Mapped to "stereotype has a header color" (old `overrideHeader`/`st.header` path); a plain node uses `--band`. The spec exposes Band/Fill as a design-editor choice not yet wired to per-node data, so this is the closest data-driven mapping. Confirm.
5. **`UmlNoteNode`/`UmlActorNode`/`UmlUseCaseNode`/`UmlPackageNode`** carry their own distinct shapes + Tailwind + hex literals and do NOT (fully) ride ClassifierBox. Now converted in **Task 8** (added to spec 3.1/3.2/4.1). Snag flagged there: only the use-case ellipse rides `.hud-surface` cleanly; the folded-corner note and stickman actor need bespoke token recipes because `clip-path` / a bare SVG glyph fight the rectangular masked `::before` frame + `box-shadow` glow (solved with a two-layer clip-path frame + `drop-shadow` glow for the note, stroke recolor + `drop-shadow` for the actor). The package only converts its tab; its box body rides the Task-3 ClassifierBox.
