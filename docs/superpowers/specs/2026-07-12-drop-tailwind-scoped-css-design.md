# Drop Tailwind for scoped CSS + token layer

**Date:** 2026-07-12
**Status:** Not implemented. `tailwindcss`/`postcss`/`autoprefixer` are still
`packages/web` deps, `tailwind.config.js`/`postcss.config.js` still exist,
`app.css` still has `@tailwind base/components/utilities`, and no
`packages/web/src/styles/` directory or `<style>` blocks exist — 0 of 44
Svelte components have a scoped `<style>` block.
**Package:** `packages/web` (the only Tailwind consumer in the monorepo)

## Problem

`packages/web` styles every component with Tailwind utility classes — 325
`class=` usages across 32 Svelte components, zero `<style>` blocks. The
markup carries long utility strings (`text-[13px] font-[550] border
border-[#d8dee8] bg-white ... hover:bg-[#f1f3f7]`) that obscure structure.
The primary pain is **utility soup in markup**, not tooling or design
constraints.

Goal: move styling out of markup into scoped `<style>` blocks backed by a
small shared token + component-class layer, and remove Tailwind entirely.

## Scope

**In scope**
- Replace Tailwind utilities with Svelte scoped `<style>` + a thin global
  layer, across all 32 components in `packages/web/src`.
- Introduce CSS custom-property tokens capturing the current values.
- Introduce a small set of global semantic component classes for repeated
  patterns (buttons, dialogs, menus).
- Remove Tailwind: deps, config, PostCSS config, `@tailwind` directives.

**Out of scope**
- Normalizing / tidying the token scale (odd arbitrary px values are kept
  as-is for now). A separate follow-up pass may round to a clean scale.
- Repointing `canvas.css` xyflow overrides to tokens (they keep raw hex).
- Any change outside `packages/web`.

## Constraints & decisions

| Decision | Choice |
|----------|--------|
| Replacement system | Svelte scoped `<style>` blocks |
| Shared layer | CSS custom-property tokens **plus** a few global component classes |
| Visual fidelity | Pixel-exact parity now; normalize later (separate work) |
| Rollout | Leaf-up incremental; each commit green; Tailwind removed in final phase |
| Global class naming | Plain semantic (`.btn-primary`), no BEM |

## Architecture

New directory `packages/web/src/styles/`:

- **`reset.css`** — small reset that reproduces the parts of Tailwind's
  preflight the app relies on: `box-sizing: border-box`, zeroed default
  margins, `border: 0 solid` default. Shipped in phase 0 so that removing
  Tailwind's preflight in the final phase is a visual no-op. This is the
  highest-risk gotcha; without it, spacing and borders shift when Tailwind
  leaves.

- **`tokens.css`** — CSS custom properties on `:root`, holding the exact
  current values (parity now). Includes:
  - Brand: `--c-brand: #1e88e5`, `--c-brand-hover: #1976d2`,
    `--c-brand-tint: #e6f1fb`, `--c-brand-tint-hover: #d8e8f9`.
  - Neutral/border: `--c-border: #d8dee8`, `--c-border-faint: #eef1f5`,
    `--c-hover-bg: #f1f3f7`, and the slate scale in use
    (`--c-slate-900/700/600/400`).
  - Danger: `--c-danger: #dc2626`, `--c-danger-hover: #b91c1c`,
    `--c-danger-bg: #fdf2f2`, `--c-danger-border: #f4caca`,
    `--c-danger-text: #7f1d1d`.
  - Alpha variants expressed as `rgba()` tokens (e.g. `--c-overlay:
    rgba(0,0,0,0.4)` for `bg-black/40`, brand-at-70/60 for
    `#1e88e5/70`, `ring-[#1e88e5]/60`).
  - Type/space/radius/shadow: the real arbitrary values as tokens
    (`--fs-13: 13px`, `--pad-btn-y: 7px`, `--radius-lg: 8px`,
    `--shadow-menu: 0 8px 24px rgba(15,23,42,0.18)`, etc). Odd values are
    preserved deliberately.

- **`components.css`** — thin global semantic classes for the repeated
  patterns present in the code:
  - Buttons: `.btn` (shared shape), `.btn-primary` (brand fill),
    `.btn-secondary` (bordered white), `.btn-danger` (red).
  - Dialogs: `.dialog-overlay` (fixed inset, overlay bg, centering),
    `.dialog-card` (white rounded shadow panel).
  - Menus: `.menu` (dropdown container), `.menu-item`.

**`app.css`** import order becomes:
`reset.css` → `tokens.css` → `components.css` → `@xyflow/svelte` style →
`canvas.css`. The `@tailwind base/components/utilities` directives stay in
place until the final phase. The existing `.keyhint` reveal rules stay
(already plain CSS).

**Per-component `<style>`** blocks hold one-off, component-specific layout.
Everything referencing shared values pulls from tokens; repeated widgets use
the global component classes.

## Migration mechanics (per component)

1. Replace the Tailwind class string with semantic class name(s).
2. Add a scoped `<style>` block for one-off layout, referencing tokens.
3. Reuse global component classes for buttons/dialogs/menus.
4. Convert dynamic/conditional styling from interpolated Tailwind strings
   (`{active ? 'text-[#1e88e5]' : 'text-slate-900'}`) to Svelte `class:`
   directives or `data-*` attributes with matching scoped selectors.

## Gotchas (identified from the real code)

- **Preflight reset** — handled by `reset.css` from phase 0 (see above).
- **lucide-svelte icons** receive color via a `class` prop
  (`text-[#1e88e5]/70`); icons render `currentColor`. Replace with a scoped
  class or inline `style` that sets `color`.
- **Alpha colors** (`bg-black/40`, `ring-[#1e88e5]/60`, `text-*/70`) become
  `rgba()` tokens.
- **`canvas.css`** xyflow `!important` overrides use raw hex and are left
  untouched (out of scope).
- **`.group:hover .keyhint`** reveal logic is already plain CSS in
  `app.css`; verify it still fires after the owning components migrate.

## Rollout (leaf-up, each commit green)

- **Phase 0 — scaffold.** Add `reset.css`, `tokens.css`, `components.css`;
  wire imports. Verify parity with Tailwind still active.
- **Phase 1 — leaf components.** `KeyHint`, `InfoTip`, `ShareToast`,
  `StereotypeRow`, `AttributeRow`, `GenericNode`, `OkfNode`, `Uml*Node`,
  `ClassifierBox`, `NodePorts`, `NodePortsStub`, `RowsCompartment`,
  `ExternalRefs`, `EdgeFlag`, `RelEdge`, `AnchorEdge`.
- **Phase 2 — mid components.** `MartRow`, `Inspector`, `InspectorPanel`,
  `ObjectInspector`, `RelationshipInspector`, `AttributeEditor`, `Dock`,
  `SelectionToolbar`, `CanvasInner`, `Canvas`.
- **Phase 3 — heavy components.** Dialogs (`ClearCanvasDialog`,
  `ImportDialog`, `LibraryDialog`, `TemplateApplyDialog`, `WelcomeDialog`,
  `ShareDialog`), `TopBar`, `App`.
- **Phase 4 — remove Tailwind.** Drop `tailwindcss`, `autoprefixer`,
  `postcss` from `package.json`; delete `tailwind.config.js` and
  `postcss.config.js`; remove `@tailwind` directives from `app.css`. Final
  full parity check.

## Verification

- **Per component:** `pnpm --filter @uaml/web build` and `svelte-check`
  green.
- **Visual:** run the dev server, eyeball each migrated component against
  its prior appearance.
- **Phase 4:** full build green; screenshot-diff key screens (TopBar, a
  dialog, a canvas node) to confirm pixel parity after Tailwind removal.

## Success criteria

- No component markup carries Tailwind utility classes.
- No Tailwind/PostCSS dependency, config, or directive remains in
  `packages/web`.
- Build and `svelte-check` pass.
- UI is pixel-identical to pre-migration (parity-now decision).
