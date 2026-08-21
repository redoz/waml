# Element-Pool Rename Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Rename the misnamed `ClassifierType` enum to `ElementType` (an honest element/metaclass-kind type) and add a derived `is_classifier(&self) -> bool` predicate, without changing any serialized string value.

**Architecture:** `ClassifierType` (in `crates/waml/src/model.rs`) is the parsed `type` discriminator on a `Node`. It serializes to/from a flat string (`serde(into = "String", from = "String")`) via `as_str()`/`parse()`, and is never emitted to TypeScript as a named type — `Node.ty` is `#[tsify(type = "string")]`, so the wire contract is plain strings like `"uml.Class"` / `"Diagram"`. This is a pure internal-Rust identifier rename plus one new predicate; the wire format is untouched.

**Tech Stack:** Rust (`waml`, `waml-ops-dto`, `waml-cli`, `waml-wasm` crates; `cargo`), TypeScript monorepo (`packages/`, `pnpm`), serde flat-string serialization, tsify/wasm-bindgen codegen.

## Global Constraints

- Serialized string values MUST NOT change: `as_str()`/`parse()` continue to emit/accept exactly `"uml.Class"`, `"uml.Interface"`, `"uml.Enum"`, `"uml.DataType"`, `"uml.Package"`, `"uml.Note"`, `"uml.Association"`, `"uml.Actor"`, `"uml.UseCase"`, `"uml.Activity"`, `"uml.StateMachine"`, `"uml.Sequence"`, `"Diagram"`, and opaque `Unknown` tokens verbatim. Serialized output must be byte-identical before and after.
- No TypeScript runtime/contract change: `packages/` consumes the wire strings as plain string literals (verified: `packages/core/src/profiles/umlDomain.ts`, `packages/okf/src/types.ts`, and numerous `*.test.ts` use `"uml.Class"` string literals; no TS file references the Rust type name except one auto-generated doc-comment). The rename does not touch any TS source.
- `is_classifier()` mapping MUST be exhaustive and explicit — no `_ => true` catch-all. The `UmlMetaclass` arm is written out variant-by-variant so adding a metaclass forces a classifier decision at compile time.
- Idiomatic Rust throughout: run `cargo fmt` on touched files before every commit, and introduce no new `cargo clippy` warnings on the touched crates. The exhaustive nested match in `is_classifier()` is intentional — if clippy suggests collapsing it into `matches!(...)` or a catch-all arm, keep the explicit arms (the exhaustiveness is the point) and, if needed, silence that one lint locally with a documented `#[allow(clippy::match_like_matches_macro)]` rather than losing the compile-time decision.
- Do NOT replace the existing `!= Diagram && !Behavior(_)` node-pool filters (in `parse.rs`/`validate.rs`) with `is_classifier()`. They are a different predicate (behavior docs are excluded from the node pool today because behavior is model-AND-view; `is_classifier()` returns `true` for behaviors). Unifying them is a later slice (§7 slice 2), not this one.
- Full gate (from `.github/workflows/ci.yml`), run in this order: `cargo test --workspace` → `pnpm build:wasm` → `pnpm lint` → `pnpm build` → `pnpm -r test`.
- Do NOT edit files under `docs/` (specs and completed plans reference `ClassifierType` historically — leave them).

---

## File Structure

Modified files (all edits are token-level identifier renames except where noted):

- `crates/waml/src/model.rs` — **Task 1**: add `is_classifier()` method + inline unit test. **Task 2**: rename `ClassifierType` → `ElementType` (enum def line 589, `From` impls, `impl` block, `Node.ty` field line 676, inline tests).
- `crates/waml/tests/serde_shape.rs` — **Task 1**: add a serde wire-stability test (added green as a lock). **Task 2**: rename the type in it (asserted strings stay identical → proof of byte-identical output).
- `crates/waml/src/parse.rs` — **Task 2**: rename (import + 13 usages).
- `crates/waml/src/validate.rs` — **Task 2**: rename (import + usages, incl. public `link()` signature).
- `crates/waml/src/ops/mod.rs` — **Task 2**: rename (import + `Op` field types + helper signatures + tests).
- `crates/waml/src/okf.rs` — **Task 2**: two comment mentions updated by the same rename sweep (lines 5, 68). Line 68 is a `///` field doc that mirrors into the generated `.d.ts`; regenerated in the same task.
- `crates/waml-ops-dto/src/lib.rs` — **Task 2**: rename (import + usages + test).
- `crates/waml-cli/src/lsp/map.rs` — **Task 2**: rename (import + usage).
- `crates/waml-cli/tests/lsp_e2e.rs` — **Task 2**: one comment mention updated by the sweep (line 100).
- `packages/wasm/src/generated/waml_wasm.d.ts` — **Task 2**: REGENERATED (not hand-edited) by `pnpm build:wasm`. The only expected diff is the single doc-comment on line 236 (`ClassifierType` → `ElementType`), mirrored from `okf.rs:68`. No exported type or wire string changes.

No files are created. No files are deleted.

---

## Task 1: Add `is_classifier()` predicate to `ClassifierType` (pre-rename)

Add the predicate and a wire-stability lock test while the type still has its old name. This is independently reviewable and testable, and the wire-stability test becomes the anchor Task 2 uses to prove byte-identical serialization.

**Files:**
- Modify: `crates/waml/src/model.rs` — add method inside the existing `impl ClassifierType` block (lines 610-635, after `as_str`); add inline test inside `#[cfg(test)] mod tests` (starts line 810), after `classifier_type_round_trips_to_string` (ends line 911).
- Modify: `crates/waml/tests/serde_shape.rs` — extend the import on line 5; add one test function at end of file.

**Interfaces:**
- Produces: `pub fn is_classifier(&self) -> bool` on `enum ClassifierType`.
- Consumes: existing `ClassifierType::{Uml, Behavior, Diagram, Unknown}`, `UmlMetaclass::{Class, Interface, Enum, DataType, Package, Note, Association, Actor, UseCase}`, `BehaviorKind::{Activity, StateMachine, Sequence}`.

Steps:

- [ ] **1.1 Write the failing predicate test.** In `crates/waml/src/model.rs`, inside `#[cfg(test)] mod tests`, immediately after the `classifier_type_round_trips_to_string` test (after line 911), add:
  ```rust
      #[test]
      fn is_classifier_matches_spec_table() {
          // Genuine UML classifiers (design spec §3.1).
          assert!(ClassifierType::Uml(UmlMetaclass::Class).is_classifier());
          assert!(ClassifierType::Uml(UmlMetaclass::Interface).is_classifier());
          assert!(ClassifierType::Uml(UmlMetaclass::Enum).is_classifier());
          assert!(ClassifierType::Uml(UmlMetaclass::DataType).is_classifier());
          assert!(ClassifierType::Uml(UmlMetaclass::Actor).is_classifier());
          assert!(ClassifierType::Uml(UmlMetaclass::UseCase).is_classifier());
          assert!(ClassifierType::Uml(UmlMetaclass::Association).is_classifier());
          // Behavior ⊂ Class: all behavior classifiers qualify.
          assert!(ClassifierType::Behavior(BehaviorKind::Activity).is_classifier());
          assert!(ClassifierType::Behavior(BehaviorKind::StateMachine).is_classifier());
          assert!(ClassifierType::Behavior(BehaviorKind::Sequence).is_classifier());
          // Not classifiers.
          assert!(!ClassifierType::Uml(UmlMetaclass::Package).is_classifier());
          assert!(!ClassifierType::Uml(UmlMetaclass::Note).is_classifier());
          assert!(!ClassifierType::Diagram.is_classifier());
          assert!(!ClassifierType::Unknown("bpmn.Task".to_string()).is_classifier());
      }
  ```

- [ ] **1.2 Run it, verify it fails.** Run:
  ```
  cargo test -p waml is_classifier
  ```
  Expected: compile error `no method named `is_classifier` found for enum `ClassifierType``.

- [ ] **1.3 Implement `is_classifier()`.** In `crates/waml/src/model.rs`, inside the existing `impl ClassifierType { ... }` block, immediately after the `as_str` method's closing brace (line 634) and before the impl block's closing brace (line 635), add:
  ```rust
      /// True only for pool members that are genuine UML **Classifiers** (design
      /// spec §3.1): `Class`, `Interface`, `Enum`, `DataType`, `Actor`, `UseCase`,
      /// `Association`, and the behavior classifiers (`Behavior ⊂ Class`).
      /// `Package`, `Note`/`Comment`, `Diagram`, and unrecognized tokens are not.
      ///
      /// The `UmlMetaclass` arm is written out explicitly (no `_ =>` catch-all) so
      /// adding a metaclass forces a classifier decision here at compile time.
      pub fn is_classifier(&self) -> bool {
          match self {
              ClassifierType::Uml(mc) => match mc {
                  UmlMetaclass::Class
                  | UmlMetaclass::Interface
                  | UmlMetaclass::Enum
                  | UmlMetaclass::DataType
                  | UmlMetaclass::Actor
                  | UmlMetaclass::UseCase
                  | UmlMetaclass::Association => true,
                  UmlMetaclass::Package | UmlMetaclass::Note => false,
              },
              // Behavior ⊂ Class: Activity / Interaction (Sequence) / StateMachine
              // are all Classifiers.
              ClassifierType::Behavior(_) => true,
              ClassifierType::Diagram => false,
              ClassifierType::Unknown(_) => false,
          }
      }
  ```

- [ ] **1.4 Run the predicate test, verify pass.** Run:
  ```
  cargo test -p waml is_classifier
  ```
  Expected: `test model::tests::is_classifier_matches_spec_table ... ok`, `test result: ok. 1 passed`.

- [ ] **1.5 Add the wire-stability lock test.** In `crates/waml/tests/serde_shape.rs`, change the import on line 5 from:
  ```rust
  use waml::model::{AssocName, ClassifierType, Model, Node, UmlMetaclass, Visibility};
  ```
  to:
  ```rust
  use waml::model::{AssocName, BehaviorKind, ClassifierType, Model, Node, UmlMetaclass, Visibility};
  ```
  Then append this test function at the end of the file:
  ```rust
  #[test]
  fn classifier_type_wire_strings_are_stable() {
      assert_eq!(
          serde_json::to_string(&ClassifierType::Uml(UmlMetaclass::Class)).unwrap(),
          "\"uml.Class\""
      );
      assert_eq!(
          serde_json::to_string(&ClassifierType::Behavior(BehaviorKind::Activity)).unwrap(),
          "\"uml.Activity\""
      );
      assert_eq!(
          serde_json::to_string(&ClassifierType::Diagram).unwrap(),
          "\"Diagram\""
      );
      assert_eq!(
          serde_json::to_string(&ClassifierType::Unknown("bpmn.Task".to_string())).unwrap(),
          "\"bpmn.Task\""
      );
      // Deserialize round-trips through `From<String>`.
      let ct: ClassifierType = serde_json::from_str("\"uml.Class\"").unwrap();
      assert_eq!(ct, ClassifierType::Uml(UmlMetaclass::Class));
  }
  ```
  (This test characterizes current behavior and passes on first run — it is the byte-identical anchor for Task 2, not a red test.)

- [ ] **1.6 Run the wire-stability test, verify pass.** Run:
  ```
  cargo test -p waml --features serde --test serde_shape classifier_type_wire_strings_are_stable
  ```
  Expected: `test classifier_type_wire_strings_are_stable ... ok`, `test result: ok. 1 passed`.

- [ ] **1.7 Run the full gate.** Run in order and confirm each passes:
  ```
  cargo test --workspace
  pnpm build:wasm
  pnpm lint
  pnpm build
  pnpm -r test
  ```
  Expected: all green (no TS or wasm change in this task; the wasm/lint/build/test steps should show no diff and pass unchanged).

- [ ] **1.8 Format, then commit.** Run:
  ```
  cargo fmt
  git add crates/waml/src/model.rs crates/waml/tests/serde_shape.rs
  git commit -m "feat(model): add ClassifierType::is_classifier predicate"
  ```
  (`cargo fmt` should report no changes — the added method/test already match rustfmt style — but run it so the commit is guaranteed idiomatic-formatted.)

---

## Task 2: Rename `ClassifierType` → `ElementType` across the workspace

Mechanical, atomic identifier rename. Because the type is referenced across four crates, the rename MUST land in a single commit or the workspace will not compile. The wire-stability test from Task 1 (renamed here) asserts the exact same strings, proving byte-identical serialization.

**Files (every `ClassifierType` occurrence; all are token renames):**
- `crates/waml/src/model.rs` — enum def (589), `From<ClassifierType> for String` / `From<String> for ClassifierType` impls (597-608), `impl` block header + arms (610-635) + the `is_classifier` body added in Task 1, `Node.ty` field type (676), inline tests (846-847, 850-851, 853-854, 859-862, 889-890, 892, 894-895, 898-899, 905-906, 908, 921) and the `is_classifier_matches_spec_table` test.
- `crates/waml/src/parse.rs` — import (13), usages (265, 414, 426, 614, 642, 643, 693, 699, 700, 782, 942) and test-module usages (1280, 1297).
- `crates/waml/src/validate.rs` — import (4), `link()` signature (153), usages (160, 164, 215, 218, 219, 391).
- `crates/waml/src/ops/mod.rs` — import (2), `Op` field types (89, 101), helper signatures (589, 627), test-module usages (729, 938, 946, 953, 1069, 1075).
- `crates/waml/src/okf.rs` — comment mentions (5 `//!`, 68 `///`).
- `crates/waml/tests/serde_shape.rs` — import (5), usages (81, 105) and the two tests added/edited in Task 1.
- `crates/waml-ops-dto/src/lib.rs` — import (3), usages (326, 345) and test-module usages (606, 614).
- `crates/waml-cli/src/lsp/map.rs` — import (6), usage (36).
- `crates/waml-cli/tests/lsp_e2e.rs` — comment mention (100).
- `packages/wasm/src/generated/waml_wasm.d.ts` — REGENERATED by `pnpm build:wasm` (expected diff: line 236 comment only).

**Interfaces:**
- Produces: `pub enum ElementType { Uml(UmlMetaclass), Behavior(BehaviorKind), Diagram, Unknown(String) }` with `ElementType::parse(&str) -> ElementType`, `ElementType::as_str(&self) -> String`, `ElementType::is_classifier(&self) -> bool`, `From<ElementType> for String`, `From<String> for ElementType`; `Node.ty: ElementType`; `pub fn link(docs: &[(String, ElementType, Document)]) -> Vec<Diagnostic>`.
- Consumes: nothing new. `UmlMetaclass`, `BehaviorKind` are unchanged.

Steps:

- [ ] **2.1 Write the failing (compile-fail) rename anchor.** Edit ONLY `crates/waml/tests/serde_shape.rs`: rename the two identifiers `ClassifierType` → `ElementType` in that file (the import on line 5 and every usage, including the `classifier_type_wire_strings_are_stable` test body and the two node literals on lines 81 and 105). Do NOT touch any other file yet. The asserted wire strings (`"uml.Class"`, `"uml.Activity"`, `"Diagram"`, `"bpmn.Task"`) stay exactly as written.

- [ ] **2.2 Run it, verify it fails to compile.** Run:
  ```
  cargo test -p waml --features serde --test serde_shape
  ```
  Expected: compile error `cannot find type `ElementType` in module `waml::model`` (and/or `unresolved import `waml::model::ElementType``). This proves the target name does not yet exist.

- [ ] **2.3 Perform the mechanical rename across all Rust source.** Prefer an editor/rust-analyzer "rename symbol" if available (compiler-aware, safest). Otherwise, from the repo root using the Bash tool (Git Bash), replace the whole **word** `ClassifierType` with `ElementType` in every `.rs` file under `crates/` — the `\b` word boundaries prevent clipping any identifier that merely contains the token as a substring (this also updates the comment mentions in `okf.rs` and `lsp_e2e.rs`, and re-covers `serde_shape.rs` idempotently):
  ```
  grep -rl 'ClassifierType' crates --include='*.rs' | xargs sed -i 's/\bClassifierType\b/ElementType/g'
  ```
  Then verify no Rust references remain:
  ```
  grep -rn 'ClassifierType' crates --include='*.rs'
  ```
  Expected: no output (zero matches).

- [ ] **2.4 Format and compile-check the rename (fast idiomatic feedback before the full gate).** Run:
  ```
  cargo fmt
  cargo check --workspace --all-targets
  ```
  Expected: `cargo fmt` reports nothing (rename is token-for-token, so formatting is unchanged); `cargo check` succeeds with no errors. A `cannot find type` error here means a `ClassifierType` reference was missed — re-run 2.3's grep to locate it.

- [ ] **2.5 Confirm the enum and impl now read as expected.** The definition in `crates/waml/src/model.rs` must now be:
  ```rust
  pub enum ElementType {
      Uml(UmlMetaclass),
      Behavior(BehaviorKind),
      Diagram,
      Unknown(String),
  }
  ```
  with `impl From<ElementType> for String`, `impl From<String> for ElementType`, `impl ElementType { pub fn parse(...) -> ElementType { ... } pub fn as_str(&self) -> String { ... } pub fn is_classifier(&self) -> bool { ... } }`, and `Node.ty: ElementType`. (No manual edit needed if 2.3 reported zero matches; this is a read-only confirmation.)

- [ ] **2.6 Run the workspace tests, verify green + byte-identical wire.** Run:
  ```
  cargo test --workspace
  ```
  Expected: all pass, including `element_type` /`is_classifier_matches_spec_table` and `classifier_type_wire_strings_are_stable` (its function name is unchanged; only the type inside it was renamed) — the latter still asserting `"uml.Class"`, `"uml.Activity"`, `"Diagram"`, `"bpmn.Task"`, proving serialization is byte-identical to pre-rename.

- [ ] **2.7 Regenerate the wasm bindings.** Run:
  ```
  pnpm build:wasm
  ```
  Expected: success. Then confirm the ONLY change to generated TS is the mirrored doc-comment:
  ```
  git diff --stat packages/wasm/src/generated/waml_wasm.d.ts
  ```
  Expected: `packages/wasm/src/generated/waml_wasm.d.ts` shows a 1-line change (line 236: `NOT the UML \`ClassifierType\`` → `NOT the UML \`ElementType\``). No exported type names or wire strings change.

- [ ] **2.8 Run the rest of the gate, verify pass.** Run in order:
  ```
  pnpm lint
  pnpm build
  pnpm -r test
  ```
  Expected: all green. TS tests pass unchanged (they consume plain wire strings, which did not change).

- [ ] **2.9 Commit.** Run:
  ```
  git add crates/waml/src/model.rs crates/waml/src/parse.rs crates/waml/src/validate.rs crates/waml/src/ops/mod.rs crates/waml/src/okf.rs crates/waml/tests/serde_shape.rs crates/waml-ops-dto/src/lib.rs crates/waml-cli/src/lsp/map.rs crates/waml-cli/tests/lsp_e2e.rs packages/wasm/src/generated/waml_wasm.d.ts
  git commit -m "refactor(model): rename ClassifierType to ElementType"
  ```

---

## Notes / risks

- **No existing `is_classifier`-style predicate to unify.** The closest existing checks are (a) the node-pool filter `p.ty != ClassifierType::Diagram && !matches!(p.ty, ClassifierType::Behavior(_))` (`parse.rs:642-643`, `validate.rs:160`) and (b) the recognized-type check `!matches!(ClassifierType::parse(ty), ClassifierType::Unknown(_))` (`parse.rs:265`, `lsp/map.rs:36`, `validate.rs`). Both are semantically distinct from `is_classifier()` and are intentionally left untouched in this slice.
- **`is_classifier()` for `Interface`/`Enum`/`DataType`** is set to `true` (UML: all are Classifiers) even though the spec §3.1 table only enumerates `Class`/`Actor`/`UseCase` — those three are illustrative, and the mapping is explicit so a future reviewer can adjust. `Package` and `Note` are `false`; `Unknown` is `false` (cannot assert an unrecognized token is a classifier).
- **`pnpm build:wasm` requires the wasm toolchain** (`wasm-pack`, `wasm32-unknown-unknown` target) — same as every other change in this repo and part of the CI gate.
