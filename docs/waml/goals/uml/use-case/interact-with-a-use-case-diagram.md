# Interact with a Use Case Diagram

**Goal:** A reader uses the WAML editor and renderer as a product feature for a
use-case diagram.

**Why:** The goal tree must distinguish the editor and renderer feature from
the permanent product actor and workflow model.

**Done when:** The product has a frozen, shipped interaction contract for a
dedicated use-case diagram view.

**Status:** planned
**MVP:** no

## Current product state

- WAML parses `uml.Actor` and `uml.UseCase` documents and keeps them as
  different model kinds.
- The editor assigns actors and use cases to different structural style
  buckets. It has no frozen row for a dedicated use-case diagram interaction.
- This document does not invent a stable scenario identifier or a GWT
  contract.

## Evidence

- Model support is covered by
  `crates/waml/src/model.rs::actor_and_usecase_metaclasses_parse_and_round_trip`.
- Current editor style classification is covered by
  `crates/waml-editor/src/node_style.rs::actor_and_usecase_are_distinct_buckets`.

## Notes

- Task 12 must reconcile the missing frozen ownership before this leaf can own
  a stable shipped scenario.
- This goal does not create or copy actors, workflows, or system-boundary
  views from `docs/waml/use-cases/**`. Task 11 owns that permanent product
  model.
- This goal does not constrain actor or use-case shapes, system-boundary
  rendering, specialized layout, geometry, or routes. It adds no specialized
  rendering feature-gap entry.
