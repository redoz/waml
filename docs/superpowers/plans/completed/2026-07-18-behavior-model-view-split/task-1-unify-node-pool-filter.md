# Behavior Model/View Split — Task 1: Unify the node-pool filter behind ElementType::is_view()

> **Segment 1 of 4** of the **Behavior Model/View Split** plan. See [`README.md`](README.md) for the plan Goal, Architecture, Tech Stack, File Structure, and Notes/risks; full original monolithic plan preserved verbatim as [`_source.md`](_source.md).
> **REQUIRED SUB-SKILL:** superpowers:subagent-driven-development (recommended) or superpowers:executing-plans. Steps use checkbox (`- [ ]`) syntax for tracking.

## Global Constraints

- **Storage format is frozen.** Do NOT touch `crates/waml/src/grammar.rs`, `crates/waml/src/syntax.rs`, or `crates/waml/src/serialize.rs`. The markdown ↔ AST round-trip (the `flow_document_serialize_is_a_semantic_fixpoint` test) must stay byte-identical. This slice reshapes only the RESOLVED runtime `Model` (design spec §9).
- **Do NOT touch the sequence/interaction substrate.** `SequenceDoc`, `Lifeline`, `SeqItem`, `SeqOperand`, `MessageVerb`, `FragmentKind`, `build_interactions`, and `Model.interactions` are a SEPARATE plan being written in parallel. Leave them exactly as they are.
- **`ops` is out of scope for flow.** `crates/waml/src/ops/mod.rs` has zero flow references (verified) — activity nodes/edges are derived-only, never mutated by `Op`s. Do not add flow ops.
- **`FlowEdgeKind` and `is_view()` matches MUST be exhaustive and explicit** — no `_ =>` catch-all where a metaclass/kind decision should be forced at compile time.
- **`is_view()` replaces both `!= Diagram && !Behavior(_)` filters** (in `parse.rs::build_model` and `validate.rs::link`). `is_view()` returns `true` for `Diagram` and every `Behavior(_)`, so the filter behavior is identical — this is a pure clarity refactor. Do NOT confuse `is_view()` with `is_classifier()` (which returns `true` for behaviors).
- **Wire naming:** new multi-word wire fields use camelCase via `#[serde(rename = ...)]` — `activityNodes`, `flowEdges`, `objectRef`, `toRef`, `controlFlow` / `objectFlow`. This matches the existing camelCase wire conventions (`objectRef`, `toRef` already exist on the old shapes).
- Idiomatic Rust: run `cargo fmt` on touched files before every commit; introduce no new `cargo clippy` warnings on the `waml` crate.
- Full CI gate (from `.github/workflows/ci.yml`), in order: `cargo test --workspace` → `pnpm build:wasm` → `pnpm lint` → `pnpm build` → `pnpm -r test`.
- **Cross-language atomicity note:** the runtime-model wire SHAPE changes, so Rust and TypeScript cannot both be green on the *full* gate at every intermediate commit. Each task runs the per-language gate that covers its own change (stated per task); the FULL gate is green only at the end of **Task 4**. This is expected for a feature branch and mirrors how a shape change lands.
- Do NOT edit files under `docs/` (historical specs/plans reference the old shapes — leave them).
- Frequent commits, one deliverable per task.

---
## Task 1: Unify the node-pool filter behind `ElementType::is_view()`

Add the honest predicate the "behavior split" slice was waiting on, and route both `!= Diagram && !Behavior(_)` filters through it. Behavior is unchanged (`is_view()` ≡ `Diagram | Behavior(_)`), so this is a pure clarity refactor and the full gate stays green.

**Files:**
- Modify: `crates/waml/src/model.rs` — add `is_view()` inside `impl ElementType` (after `is_classifier`, ~line 661); add a unit test in `mod tests`.
- Modify: `crates/waml/src/parse.rs` — `build_model` filter (~lines 641-646).
- Modify: `crates/waml/src/validate.rs` — `link` keyset filter (~line 160).

**Interfaces:**
- Produces: `pub fn is_view(&self) -> bool` on `enum ElementType`.
- Consumes: existing `ElementType::{Diagram, Behavior}`, `BehaviorKind`, `UmlMetaclass`.

Steps:

- [ ] **1.1 Write the failing predicate test.** In `crates/waml/src/model.rs`, inside `#[cfg(test)] mod tests`, immediately after the `is_classifier_matches_spec_table` test (after its closing `}`), add:
  ```rust
      #[test]
      fn is_view_flags_diagrams_and_behaviors() {
          // Views / notation — never pooled classifiers, never link targets.
          assert!(ElementType::Diagram.is_view());
          assert!(ElementType::Behavior(BehaviorKind::Activity).is_view());
          assert!(ElementType::Behavior(BehaviorKind::StateMachine).is_view());
          assert!(ElementType::Behavior(BehaviorKind::Sequence).is_view());
          // Pool members (classifiers, notes, unknowns) are not views.
          assert!(!ElementType::Uml(UmlMetaclass::Class).is_view());
          assert!(!ElementType::Uml(UmlMetaclass::Note).is_view());
          assert!(!ElementType::Uml(UmlMetaclass::Package).is_view());
          assert!(!ElementType::Unknown("bpmn.Task".to_string()).is_view());
      }
  ```

- [ ] **1.2 Run it, verify it fails.** Run:
  ```
  cargo test -p waml is_view
  ```
  Expected: compile error `no method named `is_view` found for enum `ElementType``.

- [ ] **1.3 Implement `is_view()`.** In `crates/waml/src/model.rs`, inside `impl ElementType { ... }`, immediately after the `is_classifier` method's closing brace (~line 661) and before the impl block's closing brace, add:
  ```rust
      /// True for element types that are **views / notation**, not pooled
      /// classifiers: `Diagram` (a class-diagram view) and every behavior kind
      /// (activity / state machine / interaction — each a view over pooled
      /// behavior elements, design spec §4). A view never contributes a
      /// classifier `Node` to `Model.nodes` and is never a relationship/link
      /// target. Distinct from `is_classifier()`, which is `true` for behaviors.
      pub fn is_view(&self) -> bool {
          matches!(self, ElementType::Diagram | ElementType::Behavior(_))
      }
  ```

- [ ] **1.4 Run the predicate test, verify pass.** Run:
  ```
  cargo test -p waml is_view
  ```
  Expected: `test model::tests::is_view_flags_diagrams_and_behaviors ... ok`.

- [ ] **1.5 Route the `build_model` classifier filter through `is_view()`.** In `crates/waml/src/parse.rs`, replace the filter closure in `build_model` (currently):
  ```rust
          .filter(|p| {
              p.ty != ElementType::Diagram
                  && !matches!(p.ty, ElementType::Behavior(_))
                  && p.slug != "index"
                  && p.slug != "log"
          })
  ```
  with:
  ```rust
          .filter(|p| !p.ty.is_view() && p.slug != "index" && p.slug != "log")
  ```

- [ ] **1.6 Route the `link` keyset filter through `is_view()`.** In `crates/waml/src/validate.rs`, in `pub fn link`, replace:
  ```rust
          if *ty != ElementType::Diagram && !matches!(ty, ElementType::Behavior(_)) {
              keyset.insert(slug);
          }
  ```
  with:
  ```rust
          if !ty.is_view() {
              keyset.insert(slug);
          }
  ```

- [ ] **1.7 Run the full gate, verify green.** Run in order and confirm each passes (behavior is unchanged, so every existing test — including the flow/interaction resolution and validation tests — stays green):
  ```
  cargo test --workspace
  pnpm build:wasm
  pnpm lint
  pnpm build
  pnpm -r test
  ```
  Expected: all green, no wasm/TS diff.

- [ ] **1.8 Format, then commit.** Run:
  ```
  cargo fmt
  git add crates/waml/src/model.rs crates/waml/src/parse.rs crates/waml/src/validate.rs
  git commit -m "refactor(model): unify node-pool filter behind ElementType::is_view"
  ```
