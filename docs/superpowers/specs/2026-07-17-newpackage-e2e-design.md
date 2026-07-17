# Design: Playwright e2e for the New Package flow

**Date:** 2026-07-17
**Status:** Approved (design)

## Goal

A real-browser regression suite that proves the New Package flow *actually* works
end-to-end — from clicking the trigger to the package/diagram/template materializing
on the canvas and in the navigator. This is the primary regression test for the flow
that landed on `main` at 33fec3c (unified NewPackageDialog: Empty + 4 diagram kinds +
4 templates, "Place in" placement, TopBar "Create new").

## Harness

- **Framework:** `@playwright/test`, added as a devDependency of `packages/web`
  (colocated with the app it drives; not a root concern).
- **Server:** `packages/web/playwright.config.ts` declares a `webServer` that serves
  the **production bundle** — build then `vite preview` (baseURL
  `http://localhost:4173`). This exercises the exact bundle `pnpm build` ships, so
  wasm/dist wiring breakage is caught (the class of bug that bit the display-persist
  path). One Chromium project.
- **Isolation:** Playwright's default fresh browser context per test gives empty
  `localStorage` each test, so every test starts from the empty-project state with no
  cross-test leakage. No manual reset needed.
- **Layout:**
  - `packages/web/e2e/*.spec.ts` — specs.
  - `packages/web/e2e/support/app.ts` — shared locators + actions
    (`openNewPackageDialog`, `addPackage({ name, placeIn, startFrom })`).
- **Scripts:** `test:e2e` in `packages/web/package.json`; a root passthrough
  (`pnpm --filter @waml/web test:e2e`).
- **Gate:** NOT part of `pnpm -r test`. Run explicitly via `pnpm test:e2e` (own CI
  step). Keeps the unit gate fast; e2e owns the browser + built-app cost.
- **Ignore:** add `playwright-report/` and `test-results/` to gitignore.

## Assertion target

The navigator tree row text is the primary, user-visible signal that a package or
diagram materialized: `NavigatorBody.svelte` renders each row as a `<button>` whose
label is `<span class="truncate flex-1">{row.title}</span>`. Assert via the row's
accessible name / visible text. Canvas node labels are a secondary signal where a
template's *contents* matter (assert expected node names appear).

## Specs (Given-When-Then)

Each spec structures its body with Playwright `test.step("Given …")` /
`test.step("When …")` / `test.step("Then …")` blocks so the HTML report reads as GWT
and a failure names the phase.

1. **`empty-package.spec.ts`**
   - Given an empty project.
   - When the user opens the New Package dialog, names it "Orders", picks "Empty", Adds.
   - Then an "Orders" package row appears in the navigator.

2. **`diagram-kinds.spec.ts`** — parametrized over the 4 kinds (domain / use-case /
   activity / sequence).
   - Given an empty project.
   - When the user adds a package starting from that diagram kind.
   - Then the seeded diagram appears (navigator row + the diagram renders on canvas).

3. **`templates.spec.ts`** — parametrized over the 4 templates.
   - Given an empty project.
   - When the user adds a package starting from that template.
   - Then the template's expected node(s) appear on the canvas / navigator.

4. **`placement-and-create-new.spec.ts`**
   - Placement: Given a project with a package, When adding a new package with "Place
     in" set to that package, Then the new package nests under it.
   - Create-new: Given a non-empty project, When the user clicks TopBar "Create new"
     and confirms the gate, Then the project resets to empty.

## Out of scope

- Cross-browser (Chromium only for now).
- Visual/screenshot regression.
- Wiring e2e into the default unit-test gate.
