---
type: Reference
title: WAML Feature Gaps
description: Language and tooling opportunities found while documenting product behavior.
sources:
  - { id: documentation-contract, resource: ./documentation-contract.md, title: Documentation Contract }
generated: { by: process:docs-audit, at: 2026-08-08T00:00:00Z }
status: stable
stale_after: 2026-11-08
---

# WAML Feature Gaps

This ledger records opportunities. It does not authorize a WAML language
change. The workarounds remain normative until an approved language change
replaces them.

Specialized actor, use-case, and system-boundary rendering is separate user
work. It is not a WAML language or documentation gap, and it is not a Task 5 or
Task 11 responsibility. Task 11 creates semantic product-use-case documents
and links only; it does not own specialized rendering.

## FG-001 — platform and capability predicates

### Problem

A scenario cannot state its platform and required capability as typed
predicates.

### Minimal desired notation

Scenario-level `platform` and `capability` predicates.

### Current workaround

Use the applicability field and prose in the scenario.

### Affected documents

- [Run in a browser](./goals/share-and-publish/run-in-a-browser.md)
- [Documentation Contract](./documentation-contract.md)

### Kind

semantics

## FG-002 — typed gestures and consumed input

### Problem

Scenarios cannot reuse a typed gesture or assert that the product consumes its
input.

### Minimal desired notation

Reusable typed gestures plus an input-consumed assertion.

### Current workaround

Describe the user gesture and its visible result in prose.

### Affected documents

- [Edit prose](./goals/author-in-the-editor/edit-prose.md)
- [Draw on the canvas](./goals/author-in-the-editor/draw-on-the-canvas.md)

### Kind

syntax

## FG-003 — view anchors and eventual draw results

### Problem

Scenarios cannot name a view anchor or state a result after one draw cycle.

### Minimal desired notation

Named view anchors plus `eventually` after one draw cycle.

### Current workaround

Describe the semantic view target and the observed post-draw result in prose.

### Affected documents

- [Fit the window](./goals/read-a-bundle/fit-the-window.md)
- [Read a diagram](./goals/read-a-bundle/read-a-diagram.md)

### Kind

semantics

## FG-004 — ordered collections and states

### Problem

Scenarios cannot assert an ordered collection or its selected state.

### Minimal desired notation

Ordered collection and state assertions.

### Current workaround

State the order and selected result in prose.

### Affected documents

- [Work with tabs](./goals/read-a-bundle/work-with-tabs.md)
- [Select and inspect](./goals/uml/shared/select-and-inspect.md)

### Kind

semantics

## FG-005 — semantic text positions and IME composition

### Problem

Scenarios cannot express semantic text positions, multi-caret actions, or IME
composition.

### Minimal desired notation

Semantic text positions, multi-caret actions, and IME composition.

### Current workaround

Describe text locations and input results in prose.

### Affected documents

- [Edit prose](./goals/author-in-the-editor/edit-prose.md)

### Kind

syntax

## FG-006 — transaction groups and saved states

### Problem

Scenarios cannot name an edit transaction or a saved-state marker.

### Minimal desired notation

Transaction groups and saved-state markers.

### Current workaround

Describe the grouped edit and saved-state result in prose.

### Affected documents

- [Save and undo](./goals/author-in-the-editor/save-and-undo.md)

### Kind

semantics

## FG-007 — semantic canvas targets and drag paths

### Problem

Scenarios cannot express a semantic canvas target or a drag path with a known
coordinate space.

### Minimal desired notation

Semantic canvas targets and coordinate-space-aware drag paths.

### Current workaround

Describe the target and visible placement result in prose without raw pointer
coordinates.

### Affected documents

- [Interact with a class diagram](./goals/uml/class/interact-with-a-class-diagram.md)
- [Draw on the canvas](./goals/author-in-the-editor/draw-on-the-canvas.md)

### Kind

syntax

## FG-008 — hit targets, tolerance, and z-order

### Problem

Scenarios cannot assert a hit target, hit tolerance, or z-order result.

### Minimal desired notation

Hit target, tolerance, and z-order assertions.

### Current workaround

Describe the semantic target and visible selection result in prose.

### Affected documents

- [Interact with a class diagram](./goals/uml/class/interact-with-a-class-diagram.md)
- [Interact with an activity diagram](./goals/uml/activity/interact-with-an-activity-diagram.md)
- [Interact with a sequence diagram](./goals/uml/sequence/interact-with-a-sequence-diagram.md)
- [Interact with a state-machine diagram](./goals/uml/state-machine/interact-with-a-state-machine-diagram.md)

### Kind

semantics

## FG-009 — component ports and transactions

### Problem

Architecture views cannot express component ports, asynchronous work, or a
compare-and-swap transaction explicitly.

### Minimal desired notation

Component ports plus explicit asynchronous and compare-and-swap notation.

### Current workaround

Use relationship labels and prose notes in architecture views.

### Affected documents

- [Crate ownership](./architecture/views/crate-ownership.md)
- [Editor ownership](./architecture/views/editor-ownership.md)
- [Revisioned edit transaction](./architecture/views/revisioned-edit-transaction.md)

### Kind

syntax

## FG-010 — scenario-to-evidence traceability

### Problem

WAML does not enforce complete links from a scenario identifier through a
product use case to its tests and evidence.

### Minimal desired notation

Traceable links from scenario identifiers through product use cases to tests
and evidence.

### Current workaround

Use scenario headings, marked tests, exact evidence references, and the
contract checker. Task 11 adds the permanent product-use-case links after the
goal documents exist.

### Affected documents

- [Documentation Contract](./documentation-contract.md)
- Every goal document with scenarios.
- [Product Use Cases](./use-cases/index.md)
- [Product Workflows](./use-cases/workflows/index.md)
- [Browse the Tree](./use-cases/workflows/browse-the-tree.md)
- [Command-Line Tool](./use-cases/workflows/command-line-tool.md)
- [Edit Prose](./use-cases/workflows/edit-prose.md)
- [Export a Bundle](./use-cases/workflows/export-a-bundle.md)
- [Fit the Window](./use-cases/workflows/fit-the-window.md)
- [Interact with an Activity Diagram](./use-cases/workflows/interact-with-an-activity-diagram.md)
- [Interact with a Class Diagram](./use-cases/workflows/interact-with-a-class-diagram.md)
- [Interact with a Sequence Diagram](./use-cases/workflows/interact-with-a-sequence-diagram.md)
- [Language Server](./use-cases/workflows/language-server.md)
- [Navigate and Return](./use-cases/workflows/navigate-and-return.md)
- [Open a Bundle](./use-cases/workflows/open-a-bundle.md)
- [Publish a Site](./use-cases/workflows/publish-a-site.md)
- [Read a Document](./use-cases/workflows/read-a-document.md)
- [Report Every Problem](./use-cases/workflows/report-every-problem.md)
- [Route the Edges](./use-cases/workflows/route-the-edges.md)
- [Run in a Browser](./use-cases/workflows/run-in-a-browser.md)
- [Save and Undo](./use-cases/workflows/save-and-undo.md)
- [Select and Inspect](./use-cases/workflows/select-and-inspect.md)
- [Sequence Language](./use-cases/workflows/sequence-language.md)
- [Serve Locally](./use-cases/workflows/serve-locally.md)
- [Share a Link](./use-cases/workflows/share-a-link.md)
- [Solve the Layout](./use-cases/workflows/solve-the-layout.md)
- [Text Editor Integration](./use-cases/workflows/text-editor-integration.md)
- [Use the Shell](./use-cases/workflows/use-the-shell.md)
- [Work with Tabs](./use-cases/workflows/work-with-tabs.md)

### Kind

tooling
