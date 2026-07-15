# Orders UML template split: Activity, Sequence, Use-Case

## Problem

The template library ships a single Orders Domain bundle that mixes a class
diagram (`order.md`, `customer.md`, ...) with an activity diagram
(`checkout.md`) inside one `Template` entry. Users can't pick "I want an
activity diagram" independently of "I want a domain model" — they get both or
neither. There's also no Sequence or Use-Case example anywhere in the
library, despite the parser/renderer already supporting both (`uml.Sequence`
interaction docs, `uml.Actor`/`uml.UseCase` classifiers).

Separately: when a template bundle's docs are entirely behavioral
(`uml.Activity`/`uml.Sequence`, no `uml.Class` diagram members), the canvas
lands on an empty synthetic "All" view instead of the flow/sequence view,
because `activeDiagramKey`'s default is computed from `effectiveDiagrams()`
only and never reconsidered when a template is applied.

## Goals

- Split the existing bundle into four independent `Template` entries, all in
  the same Orders domain, so the library offers one per diagram kind: Domain
  Model (class), Checkout (activity), Checkout (sequence), Use Cases.
- Each new/changed bundle is UML-accurate: activity/sequence diagrams carry
  real `uml.Class`/`uml.Actor` nodes (object nodes / lifelines), not just
  prose steps.
- A template that opens with only behavioral content lands on that content by
  default, not an empty canvas.

## Non-goals

- No changes to the parser, renderer, or profile system — Sequence,
  Actor/UseCase, Includes/Extends relationships, and the `Diagram` doc type
  already exist and are unchanged by this work.
- No new UML metaclasses or relationship kinds.
- No changes to how templates are merged into an existing model (`mode:
  "merge"`) — the landing-view fix only affects the fresh-replace path.

## Design

### Template bundle layout

Four self-contained folders under `packages/core/src/templates/`, each its
own `.okf` bundle, each its own `Template` entry in `TEMPLATES`:

| Folder | Template id | Kind | Contents |
|---|---|---|---|
| `orders-domain-uml/` (existing, `checkout.md` removed) | `uml_orders_domain` (unchanged) | `uml.Class` | order, customer, order-line, money, address, order-status, pricing-service + curated `Diagram` doc |
| `orders-checkout-activity/` (new) | `uml_orders_checkout_activity` | `uml.Activity` | `checkout.md` (moved, unchanged) + `order.md` (copied, unchanged) |
| `orders-checkout-sequence/` (new) | `uml_orders_checkout_sequence` | `uml.Sequence` | `customer.md` (new, `uml.Actor`), `order.md` (copied), `pricing-service.md` (copied), `place-order.md` (new, `uml.Sequence`) |
| `orders-use-cases/` (new) | `uml_orders_use_cases` | `uml.UseCase`/`uml.Actor` | `customer.md` (new, `uml.Actor`), 4 `uml.UseCase` docs, curated `Diagram` doc |

`uml_orders_domain`'s id is a public deep-link target (`?template=<id>`) and
stays immutable. The three new ids are free (nothing currently uses them).

Docs are duplicated across folders rather than shared, matching how the
existing OKF bundle format works (each `Template.bundle` is a flat, complete
`[path, markdown][]` — there's no cross-bundle `describes` resolution) and
how the codebase's own test fixtures already model the same entity
differently per diagram kind (a `uml.Class` Customer for structure vs. a
`uml.Actor` Customer for behavior, per `crates/waml/tests/serde_shape.rs` and
`validate.rs`).

### Activity template: object nodes

UML Activity diagrams distinguish **action nodes** (steps) from **object
nodes** (a classifier instance flowing between actions). `checkout.md`
already expresses this: its actions are section headings, and
`carries [Order](./order.md)` is the object-node link. The only change
needed is making `order.md` travel with `checkout.md` into its own bundle so
that link resolves. No content changes to either file.

### Sequence template: `place-order.md`

New `uml.Sequence` doc, same checkout story as `checkout.md` but told as an
interaction:

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

`customer.md` in this folder is a `uml.Actor` (the person interacting with
the system), distinct from the domain template's `uml.Class` Customer (the
persisted entity) — same split the parser's own test fixtures use. `order.md`
and `pricing-service.md` are verbatim copies of the domain template's files.

Scope call: this sequence only models the payment `alt`, not the in-stock
check `checkout.md` also has. Full 1:1 parity between the two diagrams isn't
necessary — the activity diagram already covers the stock path, and a denser
sequence diagram would hurt legibility for a template meant to demonstrate
the format, not exhaustively re-derive the activity diagram.

### Use-Case template

Four `uml.UseCase` docs plus one `uml.Actor`:

- `customer.md` — `uml.Actor`, "Customer".
- `place-order.md` — `uml.UseCase`, "Place Order". Relationships: `associates
  [Customer](./customer.md)`, `includes [Authenticate](./authenticate.md)`.
- `authenticate.md` — `uml.UseCase`, "Authenticate". No relationships (target
  of the `includes` above).
- `track-order.md` — `uml.UseCase`, "Track Order". Relationships: `associates
  [Customer](./customer.md)`.
- `cancel-order.md` — `uml.UseCase`, "Cancel Order". Relationships:
  `associates [Customer](./customer.md)`, `extends [Place Order](./place-order.md)`.
- `orders-use-cases.md` — curated `Diagram` doc, `profile: "uml-domain"`,
  grouped members:

```markdown
## Members

### Actors
- [Customer](./customer.md)

### Use Cases
- [Place Order](./place-order.md)
- [Authenticate](./authenticate.md)
- [Track Order](./track-order.md)
- [Cancel Order](./cancel-order.md)
```

This exercises `RelationshipKind::Includes`/`Extends` and the Actor↔UseCase
ends-less `associates` special case, all already implemented and validated
(`validate.rs:211-238`, `809-818`).

### Landing-view default fix

`packages/web/src/components/canvas/CanvasInner.svelte:99` picks
`effectiveDiagrams($model)[0].key` for the initial `activeDiagramKey`, and
nothing ever revisits that choice when a template replaces the model
(`applyTemplate`/`loadBundleWithLayout`, lines 573-576).

Add a helper, colocated with `effectiveDiagrams` in
`packages/core/src/state/diagrams.ts`:

```ts
export function defaultDiagramKey(g: Model): string {
  if (g.diagrams?.length) return g.diagrams[0].key;
  if (g.flows?.length) return g.flows[0].key;
  if (g.interactions?.length) return g.interactions[0].key;
  return effectiveDiagrams(g)[0].key; // synthetic "All"
}
```

Use it in two places in `CanvasInner.svelte`:

1. The initial `activeDiagramKey` state (line 99), replacing the
   `effectiveDiagrams($model)[0].key` fallback.
2. Inside `applyTemplate`'s `"replace"` branch (and `handleImportConfirm`'s
   `"replace"` branch, which shares the same `loadBundleWithLayout` call) —
   after the fresh model loads, set `activeDiagramKey =
   defaultDiagramKey(store.get())`.

`"merge"` mode is untouched: merging a template into existing work must not
hijack the user's current view.

### `templates/index.ts`

Four exports, one `TEMPLATES` array:

```ts
export const ordersDomain: Template = { id: "uml_orders_domain", ... };
export const ordersCheckoutActivity: Template = { id: "uml_orders_checkout_activity", ... };
export const ordersCheckoutSequence: Template = { id: "uml_orders_checkout_sequence", ... };
export const ordersUseCases: Template = { id: "uml_orders_use_cases", ... };

export const TEMPLATES: Template[] = [
  ordersDomain,
  ordersCheckoutActivity,
  ordersCheckoutSequence,
  ordersUseCases,
];
```

All four `category: "dataset"` (matches the existing entry; `LibraryDialog`
already renders a flat list, not an industry/dataset split, per the prior
session's commit).

### Bundle generation

`scripts/gen-template-bundles.mjs`'s `bundles` array grows from one entry to
four, one per folder above, each with its own `exportName` and `out` path
(`orders-domain.bundle.ts`, `orders-checkout-activity.bundle.ts`,
`orders-checkout-sequence.bundle.ts`, `orders-use-cases.bundle.ts`). Existing
`orders-domain.bundle.ts` regenerates with `checkout.md` dropped; the other
three are new files produced by `pnpm run gen:templates` (backed by `waml
bundle`, which already validates the OKF docs it packs — a bad relationship
or malformed lifeline fails the build, not just a lint pass).

## Testing

- `pnpm run gen:templates` must succeed for all four bundles — this is the
  primary correctness signal, since `waml bundle` runs the same parser/
  validator as the live app and fails on malformed docs.
- `LibraryDialog.test.ts` / `WelcomeDialog` tests: assert all four templates
  are listed (extend existing "lists templates" style assertions, don't
  restructure).
- New test for `defaultDiagramKey` in `packages/core/src/state/diagrams.ts`
  (or its existing test file): a model with only `flows` picks the first
  flow's key; only `interactions` picks the first interaction's key; empty
  model falls back to the synthetic "All" key; explicit `diagrams` still
  wins over both.
- Manual smoke: apply each of the four templates from the library on an
  empty canvas, confirm it lands on real content (not an empty canvas) and
  the diagram switcher lists the others correctly.

## Rollout

Single branch (`orders-uml-templates`), rebased onto `main` before merge,
`main` fast-forwarded to the branch tip (no merge commit) once done — no
flag, no migration, this is additive template-library content plus one
client-side default-selection fix.
