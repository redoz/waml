# Instances + Object Diagrams — Task 1: `InstanceSpecification` metaclass + `is_classifier`

> **Segment 1 of 6** of the **Instances + Object Diagrams** plan (slug `instances-object-diagrams`). See [`README.md`](README.md) for Goal, Global Constraints, File Structure, and the full plan preserved verbatim as [`_source.md`](_source.md).
> **REQUIRED SUB-SKILL:** superpowers:subagent-driven-development (recommended) or superpowers:executing-plans. Steps use checkbox (`- [ ]`) syntax for tracking.

### Task 1: InstanceSpecification metaclass + is_classifier

Add the `InstanceSpecification` UML metaclass and pin its classifier semantics. This is a **Rust-only, self-contained** change: `UmlMetaclass` has no serde/tsify derive and `ElementType` serializes to a bare string, so **no wasm binding regenerates** and no TS changes. Independently green under `cargo test -p waml`.

**Guardrails (from the design spec):**
- `is_classifier()` returns `false` for `InstanceSpecification` — an instance is NOT a classifier (spec §3.1). Write it as an **explicit match arm** (extend the existing `Package | Note => false` arm); the `Uml(mc)` match has **no `_ =>` catch-all**, so a missing arm fails to compile.
- `is_view()` must stay `false` for it (it is a pool member, not a Diagram/Behavior view) — `is_view` is a `matches!(… Diagram | Behavior(_))`, so it is already `false`; pin it with a test.

**Files:**
- Modify: `crates/waml/src/model.rs` — `enum UmlMetaclass`, `UmlMetaclass::parse`, `UmlMetaclass::name`, `ElementType::is_classifier`, plus the tests `is_classifier_matches_spec_table` and `is_view_flags_diagrams_and_behaviors`.
- Test: `crates/waml/src/model.rs` (inline `#[cfg(test)] mod tests`).

**Interfaces:**
- Produces: `UmlMetaclass::InstanceSpecification`; `ElementType::parse("uml.InstanceSpecification") == ElementType::Uml(UmlMetaclass::InstanceSpecification)` and its `.as_str()` round-trip to `"uml.InstanceSpecification"`; `is_classifier() == false`, `is_view() == false`.
- Consumes: nothing new.

- [ ] **Step 1.1: Write the failing tests.** In `crates/waml/src/model.rs`, extend the existing `is_classifier_matches_spec_table` test (add the instance line to the "Not classifiers" block) and `is_view_flags_diagrams_and_behaviors` test (add the instance line to the "Pool members … are not views" block), and add one new round-trip test:

```rust
    #[test]
    fn instance_specification_metaclass_round_trips_and_is_not_a_classifier() {
        assert_eq!(
            ElementType::parse("uml.InstanceSpecification"),
            ElementType::Uml(UmlMetaclass::InstanceSpecification)
        );
        assert_eq!(
            ElementType::Uml(UmlMetaclass::InstanceSpecification).as_str(),
            "uml.InstanceSpecification"
        );
        // An instance is NOT a classifier (spec §3.1) and NOT a view (it is a
        // pool member).
        assert!(!ElementType::Uml(UmlMetaclass::InstanceSpecification).is_classifier());
        assert!(!ElementType::Uml(UmlMetaclass::InstanceSpecification).is_view());
    }
```

Also add, inside `is_classifier_matches_spec_table` (after the `Package`/`Note` non-classifier asserts):

```rust
        assert!(!ElementType::Uml(UmlMetaclass::InstanceSpecification).is_classifier());
```

and inside `is_view_flags_diagrams_and_behaviors` (after the `Package` non-view assert):

```rust
        assert!(!ElementType::Uml(UmlMetaclass::InstanceSpecification).is_view());
```

- [ ] **Step 1.2: Run the tests, verify they fail to compile.** Run:
  ```
  cargo test -p waml --lib instance_specification
  ```
  Expected: FAIL to compile — `UmlMetaclass::InstanceSpecification` does not exist yet.

- [ ] **Step 1.3: Add the metaclass variant + parse/name arms.** In `crates/waml/src/model.rs`, in `pub enum UmlMetaclass`, add `InstanceSpecification` after `UseCase`:

```rust
pub enum UmlMetaclass {
    Class,
    Interface,
    Enum,
    DataType,
    Package,
    Note,
    Association,
    Actor,
    UseCase,
    InstanceSpecification,
}
```

  In `UmlMetaclass::parse`, add the arm (before `_ => None`):

```rust
            "InstanceSpecification" => Some(UmlMetaclass::InstanceSpecification),
```

  In `UmlMetaclass::name` (exhaustive — no catch-all), add the arm:

```rust
            UmlMetaclass::InstanceSpecification => "InstanceSpecification",
```

- [ ] **Step 1.4: Extend the `is_classifier` explicit arm.** In `ElementType::is_classifier`, add `InstanceSpecification` to the existing non-classifier arm so it reads:

```rust
                UmlMetaclass::Package | UmlMetaclass::Note | UmlMetaclass::InstanceSpecification => {
                    false
                }
```

  (Do NOT add a `_ =>` catch-all; the point is that the compiler forced this decision.)

- [ ] **Step 1.5: Check for any other exhaustive `match` over `UmlMetaclass`.** Run:
  ```
  grep -rn "match .*mc\b\|UmlMetaclass::" crates/waml/src crates/waml-editor/src
  ```
  Only `model.rs` (`parse`/`name`/`is_classifier`) matches exhaustively; `validate.rs` uses a non-exhaustive `matches!(… Actor | UseCase)` (needs no arm). If the workspace fails to compile on a new missing arm, add the arm there — but do NOT widen `is_comm_party`'s `matches!` (an instance is not a communication party).

- [ ] **Step 1.6: Run the tests, verify green.** Run:
  ```
  cargo test -p waml
  cargo clippy -p waml --all-targets
  cargo fmt
  ```
  Expected: all green; no new clippy warnings; `cargo fmt` leaves `model.rs` formatted.

- [ ] **Step 1.7: Commit.** Run:
  ```
  git add crates/waml/src/model.rs
  git commit -F - <<'EOF'
  feat(model): add InstanceSpecification metaclass (non-classifier)

  Plan: instances-object-diagrams
  Plan-Tasks: Task 1
  EOF
  ```
