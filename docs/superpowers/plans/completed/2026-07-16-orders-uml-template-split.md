# Orders UML Template Split Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Split the single Orders Domain template bundle into four independent, diagram-kind-specific templates (Class, Activity, Sequence, Use-Case) and fix the canvas so a template with only behavioral content lands on that content instead of an empty "All" view.

**Architecture:** Each template is a self-contained folder of `.md` OKF source under `packages/core/src/templates/`, compiled to a checked-in `*.bundle.ts` by `waml bundle` (via `pnpm run gen:templates`). Docs are duplicated per folder (the OKF bundle format is a flat `[path, markdown][]` with no cross-bundle resolution). A new `defaultDiagramKey` helper in core picks the first real view (diagram → flow → interaction → synthetic "All") and is wired into `CanvasInner.svelte`'s initial state and replace path.

**Tech Stack:** Svelte 5, TypeScript, Vitest, pnpm workspaces, Rust (`waml-cli` `bundle` subcommand invoked via cargo), WASM core.

## Global Constraints

- Package manager is `pnpm` (never `npm`/`yarn`); Rust builds via `cargo`.
- All work happens on branch `orders-uml-templates`, already checked out in this worktree (`C:\dev\waml\.worktrees\tweak-element-selector`). Tasks do NOT create branches/worktrees.
- Do NOT rebase onto `main` or fast-forward `main` — the orchestrator handles integration after all tasks land.
- Commit messages follow `type(scope): summary` (e.g. `feat(templates): …`, `fix(web): …`, `test(core): …`).
- Use forward-slash paths in `git add` on Windows.
- No changes to the Rust parser/validator/profile system — `uml.Sequence`, `uml.Actor`/`uml.UseCase`, `includes`/`extends`/`associates`, and grouped `Diagram` members already exist and are validated.

## Parallelization

Tasks touching disjoint files run concurrently. Waves:

- **Wave 1 (fully parallel — Tasks 1, 2, 3, 4, 5, 6):** each touches a distinct file/folder. Task 1 deletes one file in `orders-domain-uml/`; Tasks 2–4 each create a new folder; Task 5 edits `diagrams.ts` + its test; Task 6 edits the generator script.
- **Wave 2 (parallel — Tasks 7 and 9):** Task 7 (`pnpm run gen:templates`) depends on **all** markdown tasks (1, 2, 3, 4) **and** the script edit (6). Task 9 (wire `CanvasInner.svelte`) depends only on Task 5.
- **Wave 3 (Task 8):** rewrite `index.ts` — depends on Task 7 (needs the generated bundle files to import).
- **Wave 4 (Task 10):** update `LibraryDialog.test.ts` — depends on Task 8 (TEMPLATES now has 4).
- **Wave 5 (Task 11):** manual smoke test — depends on 8, 9, 10.

Intermediate states compile: after Task 1 the committed `orders-domain.bundle.ts` is stale but still valid (its `checkout.md` entry links `./order.md`, still present); it is regenerated in Task 7. `index.ts` imports the new bundles only in Task 8, after Task 7 creates them.

---

### Task 1: Remove `checkout.md` from the domain template folder

**Files:**
- Delete: `packages/core/src/templates/orders-domain-uml/checkout.md`

**Interfaces:**
- Consumes: nothing.
- Produces: `orders-domain-uml/` no longer contains an activity doc; Task 7 will regenerate `orders-domain.bundle.ts` without it.

- [ ] **Step 1: Confirm the file exists and note its content is preserved in Task 2**
Run: `git status && git ls-files packages/core/src/templates/orders-domain-uml/checkout.md`
Expected: `checkout.md` is tracked and the tree is otherwise clean.
- [ ] **Step 2: Remove the file**
Run: `git rm packages/core/src/templates/orders-domain-uml/checkout.md`
Expected: `rm 'packages/core/src/templates/orders-domain-uml/checkout.md'`
- [ ] **Step 3: Verify the other domain docs are untouched**
Run: `git status --porcelain packages/core/src/templates/orders-domain-uml/`
Expected: only a single `D  …/checkout.md` line; no other files modified.
- [ ] **Step 4: Commit**
```bash
git add packages/core/src/templates/orders-domain-uml/checkout.md
git commit -m "refactor(templates): drop checkout activity from orders-domain-uml folder"
```

---

### Task 2: New `orders-checkout-activity/` template folder

**Files:**
- Create: `packages/core/src/templates/orders-checkout-activity/checkout.md`
- Create: `packages/core/src/templates/orders-checkout-activity/order.md`

**Interfaces:**
- Consumes: nothing.
- Produces: an activity-diagram source folder Task 6/7 will bundle as `ordersCheckoutActivityBundle`.

- [ ] **Step 1: Create `checkout.md` (verbatim move of the domain folder's activity doc)**
```markdown
---
type: "uml.Activity"
title: "Checkout"
description: "Cart to shipped order."
describes: [Order](./order.md)
---

# Checkout

## Nodes

### initial
- transitions to Add Items to Cart

### Add Items to Cart
- partition: Customer
- transitions to In Stock?

### decision In Stock?
- partition: System
- when `inStock` transitions to Reserve Stock
- else transitions to Notify Out Of Stock

### Reserve Stock
- partition: System
- transitions to Calculate Total carries [Order](./order.md)

### Calculate Total
- partition: System
- transitions to Payment Authorized? carries [Order](./order.md)

### decision Payment Authorized?
- partition: System
- when `paymentAuthorized` transitions to Place Order
- else transitions to Cancel Order

### Place Order
- partition: System
- entry: `recordOrder`
- transitions to Ship Order carries [Order](./order.md)

### Ship Order
- partition: System
- transitions to final carries [Order](./order.md)

### Notify Out Of Stock
- partition: Customer
- transitions to Cancel Order

### Cancel Order
- partition: System
- transitions to final

### final
```
- [ ] **Step 2: Create `order.md` (verbatim copy of the domain folder's `order.md`)**
```markdown
---
type: "uml.Class"
stereotype: ["aggregateRoot", "entity"]
title: "Order"
description: "A customer's placed order."
---

# Order

## Attributes
- id: OrderId
- placedAt: Timestamp
- status: [OrderStatus](./order-status.md)
- shippingAddress: [Address](./address.md) {0..1}
- total: [Money](./money.md)

## Relationships
- associates [Customer](./customer.md): 1 order to 1 customer
- composes [OrderLine](./order-line.md): 1 to 1..* lines
- depends [PricingService](./pricing-service.md)
```
> Note: this `order.md` links to `./customer.md`, `./order-line.md`, etc., which do NOT exist in this folder. That is expected and non-fatal — `waml bundle` emits at most a *warning* for an unresolved relationship target (see `crates/waml/src/validate.rs`, unresolved targets on relationships are warnings, not errors), and the activity diagram only renders the Order object node. Do not add the other docs.
- [ ] **Step 3: Commit**
```bash
git add packages/core/src/templates/orders-checkout-activity/checkout.md packages/core/src/templates/orders-checkout-activity/order.md
git commit -m "feat(templates): add orders-checkout-activity source folder"
```

---

### Task 3: New `orders-checkout-sequence/` template folder

**Files:**
- Create: `packages/core/src/templates/orders-checkout-sequence/customer.md`
- Create: `packages/core/src/templates/orders-checkout-sequence/order.md`
- Create: `packages/core/src/templates/orders-checkout-sequence/pricing-service.md`
- Create: `packages/core/src/templates/orders-checkout-sequence/place-order.md`

**Interfaces:**
- Consumes: nothing.
- Produces: a sequence-diagram source folder Task 6/7 will bundle as `ordersCheckoutSequenceBundle`.

- [ ] **Step 1: Create `customer.md` (new `uml.Actor` — the person, distinct from the domain `uml.Class` Customer)**
```markdown
---
type: "uml.Actor"
title: "Customer"
---

# Customer
```
- [ ] **Step 2: Create `order.md` (verbatim copy of the domain folder's `order.md`)**
```markdown
---
type: "uml.Class"
stereotype: ["aggregateRoot", "entity"]
title: "Order"
description: "A customer's placed order."
---

# Order

## Attributes
- id: OrderId
- placedAt: Timestamp
- status: [OrderStatus](./order-status.md)
- shippingAddress: [Address](./address.md) {0..1}
- total: [Money](./money.md)

## Relationships
- associates [Customer](./customer.md): 1 order to 1 customer
- composes [OrderLine](./order-line.md): 1 to 1..* lines
- depends [PricingService](./pricing-service.md)
```
- [ ] **Step 3: Create `pricing-service.md` (verbatim copy of the domain folder's `pricing-service.md`)**
```markdown
---
type: "uml.Interface"
stereotype: ["service"]
title: "PricingService"
---

# PricingService
```
- [ ] **Step 4: Create `place-order.md` (new `uml.Sequence` doc)**
```markdown
---
type: "uml.Sequence"
title: "Place Order"
describes: [Order](./order.md)
---

# Place Order

## Lifelines
- [Customer](./customer.md)
- [Order](./order.md) as order
- [PricingService](./pricing-service.md) as pricing

## Messages
- Customer calls order: `place(items)`
- order calls pricing: `calculateTotal(items)`
- pricing replies order: `total`
- alt
  - when `paymentAuthorized`
    - order calls order: `recordOrder()`
    - order replies Customer: `confirmation`
  - else
    - order sends Customer: `paymentFailed()`
```
> The message/fragment syntax (`calls`/`replies`/`sends`, `alt`/`when`/`else`, `as <alias>`) matches the parser's own fixture in `crates/waml/tests/serde_shape.rs`.
- [ ] **Step 5: Commit**
```bash
git add packages/core/src/templates/orders-checkout-sequence/customer.md packages/core/src/templates/orders-checkout-sequence/order.md packages/core/src/templates/orders-checkout-sequence/pricing-service.md packages/core/src/templates/orders-checkout-sequence/place-order.md
git commit -m "feat(templates): add orders-checkout-sequence source folder"
```

---

### Task 4: New `orders-use-cases/` template folder

**Files:**
- Create: `packages/core/src/templates/orders-use-cases/customer.md`
- Create: `packages/core/src/templates/orders-use-cases/place-order.md`
- Create: `packages/core/src/templates/orders-use-cases/authenticate.md`
- Create: `packages/core/src/templates/orders-use-cases/track-order.md`
- Create: `packages/core/src/templates/orders-use-cases/cancel-order.md`
- Create: `packages/core/src/templates/orders-use-cases/orders-use-cases.md`

**Interfaces:**
- Consumes: nothing.
- Produces: a use-case-diagram source folder Task 6/7 will bundle as `ordersUseCasesBundle`.

- [ ] **Step 1: Create `customer.md` (`uml.Actor`)**
```markdown
---
type: "uml.Actor"
title: "Customer"
---

# Customer
```
- [ ] **Step 2: Create `place-order.md` (`uml.UseCase`; associates Customer, includes Authenticate)**
```markdown
---
type: "uml.UseCase"
title: "Place Order"
---

# Place Order

## Relationships
- associates [Customer](./customer.md)
- includes [Authenticate](./authenticate.md)
```
- [ ] **Step 3: Create `authenticate.md` (`uml.UseCase`; no relationships — target of the `includes`)**
```markdown
---
type: "uml.UseCase"
title: "Authenticate"
---

# Authenticate
```
- [ ] **Step 4: Create `track-order.md` (`uml.UseCase`; associates Customer)**
```markdown
---
type: "uml.UseCase"
title: "Track Order"
---

# Track Order

## Relationships
- associates [Customer](./customer.md)
```
- [ ] **Step 5: Create `cancel-order.md` (`uml.UseCase`; associates Customer, extends Place Order)**
```markdown
---
type: "uml.UseCase"
title: "Cancel Order"
---

# Cancel Order

## Relationships
- associates [Customer](./customer.md)
- extends [Place Order](./place-order.md)
```
- [ ] **Step 6: Create `orders-use-cases.md` (curated `Diagram` doc, grouped members)**
```markdown
---
type: "Diagram"
title: "Orders Use Cases"
profile: "uml-domain"
---

# Orders Use Cases

## Members

### Actors
- [Customer](./customer.md)

### Use Cases
- [Place Order](./place-order.md)
- [Authenticate](./authenticate.md)
- [Track Order](./track-order.md)
- [Cancel Order](./cancel-order.md)
```
> The `### Actors` / `### Use Cases` H3s under `## Members` are member-group headings — supported by `MemberGroup` recursion in `crates/waml/src/validate.rs` (`check_group_members`, `collect_group_names`). The `associates` Actor↔UseCase line has no ends (the ends-less special case, already validated). `includes`/`extends` are the ends-less dependency verbs in `crates/waml/src/grammar.rs`.
- [ ] **Step 7: Commit**
```bash
git add packages/core/src/templates/orders-use-cases/customer.md packages/core/src/templates/orders-use-cases/place-order.md packages/core/src/templates/orders-use-cases/authenticate.md packages/core/src/templates/orders-use-cases/track-order.md packages/core/src/templates/orders-use-cases/cancel-order.md packages/core/src/templates/orders-use-cases/orders-use-cases.md
git commit -m "feat(templates): add orders-use-cases source folder"
```

---

### Task 5: Add `defaultDiagramKey` to `diagrams.ts` (with unit test)

**Files:**
- Modify: `packages/core/src/state/diagrams.ts` (add a new exported function after `effectiveDiagrams`, around line 22)
- Test: `packages/core/src/state/diagrams.test.ts` (add a new `describe` block)

**Interfaces:**
- Consumes: `effectiveDiagrams(g: ModelGraph): Diagram[]`, `ALL_DIAGRAM_KEY`.
- Produces: `export function defaultDiagramKey(g: ModelGraph): string` — used by `CanvasInner.svelte` in Task 9.

- [ ] **Step 1: Write the failing test** — append to `packages/core/src/state/diagrams.test.ts`, and update its top import to `import { effectiveDiagrams, defaultDiagramKey, ALL_DIAGRAM_KEY } from "./diagrams";`
```ts
describe("defaultDiagramKey", () => {
  it("explicit diagrams win over flows and interactions", () => {
    const g: ModelGraph = {
      nodes: [node("a")], edges: [], path: "", packages: [],
      diagrams: [{ key: "d1", title: "D", profile: "p", members: ["a"] }],
      flows: [{ key: "f1", title: "F", flavor: "activity", nodes: [], edges: [] }] as ModelGraph["flows"],
      interactions: [{ key: "s1", title: "S", lifelines: [], messages: [] }] as ModelGraph["interactions"],
    };
    expect(defaultDiagramKey(g)).toBe("d1");
  });
  it("no diagrams, has flows ⇒ first flow key", () => {
    const g: ModelGraph = {
      nodes: [node("a")], edges: [], diagrams: [], path: "", packages: [],
      flows: [{ key: "f1", title: "F", flavor: "activity", nodes: [], edges: [] }] as ModelGraph["flows"],
    };
    expect(defaultDiagramKey(g)).toBe("f1");
  });
  it("no diagrams or flows, has interactions ⇒ first interaction key", () => {
    const g: ModelGraph = {
      nodes: [node("a")], edges: [], diagrams: [], path: "", packages: [],
      interactions: [{ key: "s1", title: "S", lifelines: [], messages: [] }] as ModelGraph["interactions"],
    };
    expect(defaultDiagramKey(g)).toBe("s1");
  });
  it("no diagrams/flows/interactions ⇒ synthetic All key", () => {
    const g: ModelGraph = { nodes: [node("a")], edges: [], diagrams: [], path: "", packages: [] };
    expect(defaultDiagramKey(g)).toBe(ALL_DIAGRAM_KEY);
  });
});
```
- [ ] **Step 2: Run the test to verify it fails**
Run: `pnpm --filter @waml/core exec vitest run src/state/diagrams.test.ts`
Expected: FAIL — `defaultDiagramKey` is not exported / is not a function.
- [ ] **Step 3: Implement** — add to `packages/core/src/state/diagrams.ts` immediately after the closing brace of `effectiveDiagrams` (after line 22):
```ts
/** The view a fresh model should open on: a curated diagram, else the first
 *  behavioral flow, else the first interaction, else the synthetic "All". */
export function defaultDiagramKey(g: ModelGraph): string {
  if (g.diagrams.length) return g.diagrams[0].key;
  if (g.flows?.length) return g.flows[0].key;
  if (g.interactions?.length) return g.interactions[0].key;
  return effectiveDiagrams(g)[0].key;
}
```
- [ ] **Step 4: Run the test to verify it passes**
Run: `pnpm --filter @waml/core exec vitest run src/state/diagrams.test.ts`
Expected: PASS (all `effectiveDiagrams` and `defaultDiagramKey` cases green).
- [ ] **Step 5: Commit**
```bash
git add packages/core/src/state/diagrams.ts packages/core/src/state/diagrams.test.ts
git commit -m "feat(core): add defaultDiagramKey view-selection helper"
```

---

### Task 6: Extend the bundle generator's `bundles` array

**Files:**
- Modify: `scripts/gen-template-bundles.mjs:14-16` (the `bundles` array)

**Interfaces:**
- Consumes: nothing (edits config only).
- Produces: the generator now emits four bundle files. Running it is Task 7.

- [ ] **Step 1: Replace the `bundles` array** — change lines 14–16 to:
```js
const bundles = [
  { dir: "orders-domain-uml", exportName: "ordersDomainBundle", out: join(templatesDir, "orders-domain.bundle.ts") },
  { dir: "orders-checkout-activity", exportName: "ordersCheckoutActivityBundle", out: join(templatesDir, "orders-checkout-activity.bundle.ts") },
  { dir: "orders-checkout-sequence", exportName: "ordersCheckoutSequenceBundle", out: join(templatesDir, "orders-checkout-sequence.bundle.ts") },
  { dir: "orders-use-cases", exportName: "ordersUseCasesBundle", out: join(templatesDir, "orders-use-cases.bundle.ts") },
];
```
- [ ] **Step 2: Sanity-check the script parses (no execution)**
Run: `node --check scripts/gen-template-bundles.mjs`
Expected: exit 0, no output.
- [ ] **Step 3: Commit**
```bash
git add scripts/gen-template-bundles.mjs
git commit -m "chore(templates): generate four template bundles"
```

---

### Task 7: Run `pnpm run gen:templates` — regenerate + create the four bundles

> **Depends on Tasks 1, 2, 3, 4, 6 all being committed first.** This is the real validation gate: `pnpm run gen:templates` shells out to `cargo run -p waml-cli … bundle`, which runs the full OKF parser/validator on every doc. A malformed lifeline, bad relationship verb, or unresolved required target fails here with an exact doc/line message.

**Files:**
- Regenerate (modify): `packages/core/src/templates/orders-domain.bundle.ts` (now without checkout content)
- Create: `packages/core/src/templates/orders-checkout-activity.bundle.ts`
- Create: `packages/core/src/templates/orders-checkout-sequence.bundle.ts`
- Create: `packages/core/src/templates/orders-use-cases.bundle.ts`

**Interfaces:**
- Consumes: the four source folders + the extended generator.
- Produces: four `export const …Bundle: [string, string][]` modules imported by Task 8.

- [ ] **Step 1: Run the generator**
Run: `pnpm run gen:templates`
> **First run compiles the Rust `waml-cli` crate — this can take one to several minutes and prints cargo compilation output. It is NOT hung.** Subsequent runs are fast.
Expected: exit 0, ending with exactly four lines:
```
wrote …/orders-domain.bundle.ts
wrote …/orders-checkout-activity.bundle.ts
wrote …/orders-checkout-sequence.bundle.ts
wrote …/orders-use-cases.bundle.ts
```
If it exits non-zero, read the error — it names the offending `<dir>/<file>.md` and line. Fix the source file (in whichever Task 2–4 folder), re-commit that file, and re-run this step. Do NOT hand-edit the `.bundle.ts` outputs.
- [ ] **Step 2: Confirm the domain bundle dropped checkout**
Run: `git diff --stat packages/core/src/templates/orders-domain.bundle.ts && grep -c "orders-domain-uml/checkout.md" packages/core/src/templates/orders-domain.bundle.ts`
Expected: the file shows as modified and the grep count is `0`.
- [ ] **Step 3: Confirm the three new bundles exist and are non-empty**
Run: `git status --porcelain packages/core/src/templates/*.bundle.ts`
Expected: `orders-domain.bundle.ts` modified; the other three shown as new/untracked.
- [ ] **Step 4: Commit**
```bash
git add packages/core/src/templates/orders-domain.bundle.ts packages/core/src/templates/orders-checkout-activity.bundle.ts packages/core/src/templates/orders-checkout-sequence.bundle.ts packages/core/src/templates/orders-use-cases.bundle.ts
git commit -m "feat(templates): regenerate four OKF template bundles"
```

---

### Task 8: Rewrite `templates/index.ts` to export four templates

> **Depends on Task 7** (imports the generated bundle modules).

**Files:**
- Modify: `packages/core/src/templates/index.ts` (replace the whole file)

**Interfaces:**
- Consumes: `ordersDomainBundle`, `ordersCheckoutActivityBundle`, `ordersCheckoutSequenceBundle`, `ordersUseCasesBundle`; `Template` from `./helpers`.
- Produces: `TEMPLATES: Template[]` of length 4, and named exports `ordersDomain`, `ordersCheckoutActivity`, `ordersCheckoutSequence`, `ordersUseCases`.

- [ ] **Step 1: Replace the entire file contents** with:
```ts
// Template library. Ships four templates — one per UML diagram kind in the same
// Orders domain — each committed as an `.okf` bundle. `uml_orders_domain`'s id is
// immutable: `?template=<id>` deep links are the CTA target for the blog gallery,
// launch emails and posts. The three newer ids are free.
export type { Template } from "./helpers";

import type { Template } from "./helpers";
import { ordersDomainBundle } from "./orders-domain.bundle";
import { ordersCheckoutActivityBundle } from "./orders-checkout-activity.bundle";
import { ordersCheckoutSequenceBundle } from "./orders-checkout-sequence.bundle";
import { ordersUseCasesBundle } from "./orders-use-cases.bundle";

export const ordersDomain: Template = {
  id: "uml_orders_domain",
  nicheId: null,
  category: "dataset",
  name: "Orders Domain (UML)",
  description:
    "DDD-flavored UML domain model: aggregate root, entities, value objects, an enum and a service interface.",
  bundle: ordersDomainBundle,
};

export const ordersCheckoutActivity: Template = {
  id: "uml_orders_checkout_activity",
  nicheId: null,
  category: "dataset",
  name: "Orders Checkout (Activity)",
  description:
    "UML activity diagram of the checkout flow: actions, decisions, partitions and an Order object node.",
  bundle: ordersCheckoutActivityBundle,
};

export const ordersCheckoutSequence: Template = {
  id: "uml_orders_checkout_sequence",
  nicheId: null,
  category: "dataset",
  name: "Orders Checkout (Sequence)",
  description:
    "UML sequence diagram of placing an order: a Customer actor with Order and PricingService lifelines and a payment alt.",
  bundle: ordersCheckoutSequenceBundle,
};

export const ordersUseCases: Template = {
  id: "uml_orders_use_cases",
  nicheId: null,
  category: "dataset",
  name: "Orders Use Cases",
  description:
    "UML use-case diagram: a Customer actor with Place Order, Authenticate, Track Order and Cancel Order use cases (include / extend).",
  bundle: ordersUseCasesBundle,
};

export const TEMPLATES: Template[] = [
  ordersDomain,
  ordersCheckoutActivity,
  ordersCheckoutSequence,
  ordersUseCases,
];
```
- [ ] **Step 2: Typecheck / build the core package**
Run: `pnpm --filter @waml/core build`
Expected: exit 0 — all four bundle imports resolve, no TS errors.
- [ ] **Step 3: Commit**
```bash
git add packages/core/src/templates/index.ts
git commit -m "feat(templates): export four Orders UML templates"
```

---

### Task 9: Wire `defaultDiagramKey` into `CanvasInner.svelte`

> **Depends on Task 5.** Independent of Tasks 6–8 (touches a different file). Runs in Wave 2 parallel with Task 7.

**Files:**
- Modify: `packages/web/src/components/canvas/CanvasInner.svelte` — the diagrams import block (~lines 56–61), the initial `activeDiagramKey` state (~line 99), and `loadBundleWithLayout` (~lines 554–557).

**Interfaces:**
- Consumes: `defaultDiagramKey` from `@waml/core/state/diagrams`.
- Produces: fresh-replace loads (initial mount + template/import "replace") land on the first real view.

> Re-locate the exact lines by search (`activeDiagramKey`, `effectiveDiagrams`, `loadBundleWithLayout`) — line numbers may have shifted. The `else` (replace) branches of both `applyTemplate` and `handleImportConfirm` call `loadBundleWithLayout`; `"merge"` uses `applyMergeWithLayout` (a different function), so putting the reset inside `loadBundleWithLayout` covers exactly the replace paths and leaves merge untouched.

- [ ] **Step 1: Add `defaultDiagramKey` to the diagrams import**
Change the `import { effectiveDiagrams, ALL_DIAGRAM_KEY, loadActiveDiagramKey, persistActiveDiagramKey } from "@waml/core/state/diagrams";` block to also import `defaultDiagramKey`:
```ts
  import {
    effectiveDiagrams,
    defaultDiagramKey,
    ALL_DIAGRAM_KEY,
    loadActiveDiagramKey,
    persistActiveDiagramKey,
  } from "@waml/core/state/diagrams";
```
- [ ] **Step 2: Use `defaultDiagramKey` for the initial state**
Change the initial `activeDiagramKey` line from:
```ts
  let activeDiagramKey = $state<string>(loadActiveDiagramKey() ?? effectiveDiagrams($model)[0].key);
```
to:
```ts
  let activeDiagramKey = $state<string>(loadActiveDiagramKey() ?? defaultDiagramKey($model));
```
- [ ] **Step 3: Reset the view on fresh replace inside `loadBundleWithLayout`**
Change:
```ts
  // Replace the whole model with a bundle, then auto-layout it.
  function loadBundleWithLayout(bundle: Bundle) {
    store.load(bundle);
    layoutAll();
  }
```
to:
```ts
  // Replace the whole model with a bundle, then auto-layout it. A fresh model
  // may be purely behavioral (no curated diagram); land on its first real view
  // rather than keeping the previous model's activeDiagramKey. Merge (a
  // different code path) intentionally keeps the user's current view.
  function loadBundleWithLayout(bundle: Bundle) {
    store.load(bundle);
    activeDiagramKey = defaultDiagramKey(store.get());
    layoutAll();
  }
```
- [ ] **Step 4: Verify the web package typechecks/builds**
Run: `pnpm --filter @waml/web build`
Expected: exit 0, no svelte-check / TS errors. (Requires the WASM package built; if it errors on missing `@waml/wasm`, run `pnpm build:wasm` first, then retry.)
- [ ] **Step 5: Commit**
```bash
git add packages/web/src/components/canvas/CanvasInner.svelte
git commit -m "fix(web): land fresh-replaced models on their first real view"
```

---

### Task 10: Update `LibraryDialog.test.ts` to expect four templates

> **Depends on Task 8** (needs `TEMPLATES` to contain 4 entries). `WelcomeDialog.test.ts` has NO template-count assertion (verified: it only checks the Start-blank / Import handlers), so it needs no change.

**Files:**
- Modify: `packages/web/src/components/LibraryDialog.test.ts`

**Interfaces:**
- Consumes: `TEMPLATES` (length 4) and the `LibraryDialog` component (renders one "Use" button per template).
- Produces: regression coverage that all four templates list and roll out.

- [ ] **Step 1: Add a failing count assertion** — append this test to `packages/web/src/components/LibraryDialog.test.ts` (keep the existing "Use rolls out the first template" test unchanged):
```ts
test("lists all four templates", () => {
  render(LibraryDialog, { props: { onUse: vi.fn(), onClose: vi.fn() } });
  const useButtons = screen.getAllByRole("button", { name: /Use/ });
  expect(TEMPLATES).toHaveLength(4);
  expect(useButtons).toHaveLength(TEMPLATES.length);
});
```
- [ ] **Step 2: Run the test file to verify the new case passes and the old one still passes**
Run: `pnpm --filter @waml/web exec vitest run src/components/LibraryDialog.test.ts`
Expected: PASS — both tests green. (`LibraryDialog` derives a preview graph per template via `build_model`; four green "Use" buttons means all four bundles are WASM-valid too.)
- [ ] **Step 3: Run the full web + core test suites to confirm no fallout**
Run: `pnpm -r test`
Expected: exit 0, all packages green (includes `diagrams.test.ts` from Task 5 and both dialog tests).
- [ ] **Step 4: Commit**
```bash
git add packages/web/src/components/LibraryDialog.test.ts
git commit -m "test(web): assert the library lists all four templates"
```

---

### Task 11: Manual smoke test (no commit)

> **Depends on Tasks 8, 9, 10.** Read-only verification; produces no file changes.

**Files:** none.

- [ ] **Step 1: Ensure WASM is built** (skip if `packages/wasm` output already present)
Run: `pnpm build:wasm`
Expected: exit 0.
- [ ] **Step 2: Start the web dev server**
Run: `pnpm dev`
Expected: Vite prints a local URL (e.g. `http://localhost:5173`). Open it in a browser on a fresh profile / cleared localStorage so the canvas starts empty.
- [ ] **Step 3: Apply each template and verify it lands on real content**
Open the Template library. Confirm it lists all four: **Orders Domain (UML)**, **Orders Checkout (Activity)**, **Orders Checkout (Sequence)**, **Orders Use Cases**. For each, on an empty canvas click **Use** and confirm:
  - **Orders Domain (UML):** class diagram renders with Order, Customer, OrderLine, etc.
  - **Orders Checkout (Activity):** lands directly on the Checkout activity flow (actions/decisions/partitions), NOT an empty "All" view.
  - **Orders Checkout (Sequence):** lands directly on the Place Order sequence (Customer/Order/Pricing lifelines, payment `alt`), NOT empty.
  - **Orders Use Cases:** lands on the Orders Use Cases diagram (Customer actor + four use cases with include/extend), NOT empty.
  Between templates, clear the canvas (or reload with cleared storage) so each applies onto an empty canvas via the fresh-replace path.
- [ ] **Step 4: Verify the diagram switcher lists the other views** for each applied template (e.g. after applying the sequence template the switcher offers its lifeline-derived views; the domain template offers its curated diagram).
- [ ] **Step 5: Stop the dev server** (Ctrl+C). No commit — this task only verifies.

---

## Self-Review

- **Spec coverage:** folder split (Tasks 1–4), generator (6) + regeneration (7), `index.ts` (8), `defaultDiagramKey` + test (5) and wiring (9), test updates (10), manual smoke (11) — every spec section mapped.
- **Placeholders:** none — every markdown doc and TS file appears verbatim.
- **Name consistency:** `defaultDiagramKey`, `ordersCheckoutActivityBundle`/`ordersCheckoutSequenceBundle`/`ordersUseCasesBundle`, and ids `uml_orders_checkout_activity`/`uml_orders_checkout_sequence`/`uml_orders_use_cases` are spelled identically across generator, bundle imports, `index.ts`, and tests.
