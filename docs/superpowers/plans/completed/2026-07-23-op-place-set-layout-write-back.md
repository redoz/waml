# Op::PlaceSet Layout Write-Back (Phase A) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add an AST-based `Op::PlaceSet` to the `waml` crate that authors a `## Layout` placement statement in-memory (parse → mutate AST → serialize), with axis-scoped replace semantics, plus the matching exhaustive-match arm in `waml-ops-dto`.

**Architecture:** `PlaceSet` is a new variant of the existing `Op` enum in `crates/waml/src/ops/mod.rs`. It dispatches through the established `edit_doc` primitive (find doc → `parse_document` → mutate `Document` → `serialize_document`). A new `layout_mut` helper (copied from the `attrs_mut`/`values_mut`/`rels_mut` pattern) returns `&mut Vec<Line<LayoutItem>>`, creating an empty `Section::Layout` if absent. Each authored direction becomes a separate 2-operand `LayoutStatement::Placement` because the grammar invariant is `directions.len() == operands.len() - 1`; a corner drop (2 directions) is therefore 2 statements. Replace is *axis-scoped*: authoring a horizontal placement (Left/Right) replaces an existing horizontal placement between the same subject/reference pair but leaves a vertical one (Above/Below) alone.

**Tech Stack:** Rust, `cargo test`, the `waml` crate's syntax AST (`crate::syntax`) and serializer (`crate::serialize`).

## Global Constraints

- Phase A is **native-only, AST-only, in-memory**. No disk writes in the op; no app/editor/UI wiring; no web DTO round-trip.
- Operands are authored as `NameRef::Link { title, slug }` — matching the mini fixture form `[Title](./slug.md)` (`crates/waml/src/layout.rs:59`). The solver resolves by **slug**; the title is cosmetic.
- Subject/reference titles and slugs are used **verbatim** from the op fields (no bundle re-resolution) so serialized output is deterministic.
- Axis mapping: **Horizontal** = `Direction::LeftOf | Direction::RightOf`; **Vertical** = `Direction::Above | Direction::Below`.
- Replace semantics = replace-only-same-reference, axis-scoped: before pushing a new direction's statement, `retain`-drop any existing 2-operand `Placement` whose operands are exactly `[subject_slug, reference_slug]` **on the same axis** as the new direction. Then push.
- `Direction` enum variants are `LeftOf, RightOf, Above, Below` (`crates/waml/src/syntax.rs:187`) — note `LeftOf`/`RightOf`, not `Left`/`Right`.
- `Line` constructor for a parsed layout bullet: `Line::Parsed(LayoutItem { line: 0, stmt })` (`crates/waml/src/syntax.rs:40`, `:17`).
- Tests live in the existing `#[cfg(test)] mod tests` at the bottom of `crates/waml/src/ops/mod.rs` and use `apply(&bundle, &[Op::PlaceSet{..}])`, matching the bundle-fixture style already there (e.g. `diagram_doc()`).
- Tasks 1-4 verify with `cargo test -p waml`. `cargo test -p waml` compiles only the `waml` crate and its dependencies — **not** the dependent `waml-ops-dto` crate — so the temporarily non-exhaustive `from_op` match in `waml-ops-dto` does not affect them. Task 5 fixes `waml-ops-dto` and verifies with `cargo build -p waml-ops-dto`.

---

## File Structure

- **Modify** `crates/waml/src/ops/mod.rs`
  - Extend the top-of-file `use crate::syntax::{...}` import to bring in the layout AST types.
  - Add the `PlaceSet { .. }` variant to the `Op` enum (`:62`).
  - Add the dispatch arm to `apply_one` (`:178`).
  - Add the `layout_mut` helper (next to `attrs_mut` at `:321`).
  - Add `op_place_set` plus the small free helpers `link_operand`, `operand_slug`, `placement_matches`, `dir_is_horizontal`.
  - Add tests to the existing `#[cfg(test)] mod tests` (`:968`).
- **Modify** `crates/waml-ops-dto/src/lib.rs`
  - Add the `Op::PlaceSet { .. } => unreachable!(...)` arm to the exhaustive `from_op` match (`:578`-`:765`), immediately before its closing brace.

---

### Task 1: `Op::PlaceSet` variant + `layout_mut` + base single-direction authoring

Introduces the variant, the `apply_one` dispatch, the `layout_mut` helper (create-if-absent), and a first `op_place_set` that authors **one** placement (`directions[0]`, no replace yet). Two tests: authoring into an existing `## Layout`, and creating the section when absent.

**Files:**
- Modify: `crates/waml/src/ops/mod.rs` (imports at `:7`; `Op` enum at `:62`; `apply_one` at `:178`; new helper near `:321`; new `op_place_set` + free helpers; tests in `mod tests` at `:968`)
- Test: `crates/waml/src/ops/mod.rs` (`#[cfg(test)] mod tests`)

**Interfaces:**
- Consumes: `edit_doc(work, slug, op, |doc| ...)`, `parse_document`, `serialize_document`, `Bundle`, `OpError`, and the syntax AST types `Document`, `Section`, `Line`, `LayoutItem`, `LayoutStatement`, `Operand`, `OperandRef`, `NameRef`, `Direction`.
- Produces:
  - `Op::PlaceSet { diagram: String, subject_title: String, subject_slug: String, reference_title: String, reference_slug: String, directions: Vec<Direction> }`
  - `pub(crate) fn layout_mut(doc: &mut Document) -> &mut Vec<Line<LayoutItem>>`
  - `fn op_place_set(work: &mut Bundle, diagram: &str, subject_title: &str, subject_slug: &str, reference_title: &str, reference_slug: &str, directions: &[Direction]) -> Result<(), OpError>`
  - `fn link_operand(title: &str, slug: &str) -> Operand`
  - Test helpers `placeset(...)`, `layout_diagram(...)`, `diagram_no_layout()`.

- [ ] **Step 1: Write the failing tests**

Add to `#[cfg(test)] mod tests` in `crates/waml/src/ops/mod.rs` (uses `Direction`, `Section`, `parse_document`, all reachable via `use super::*;`):

```rust
    // ---- Op::PlaceSet (## Layout write-back, Phase A) ----

    /// A `## Layout` diagram doc whose Layout body is `layout_body` (may be "").
    fn layout_diagram(layout_body: &str) -> Bundle {
        vec![(
            "shop/dia.md".to_string(),
            format!(
                "---\ntype: Diagram\ntitle: D\nprofile: uml-domain\n---\n# D\n\n## Layout\n{layout_body}"
            ),
        )]
    }

    /// A diagram doc with NO `## Layout` section.
    fn diagram_no_layout() -> Bundle {
        vec![(
            "shop/dia.md".to_string(),
            "---\ntype: Diagram\ntitle: D\nprofile: uml-domain\n---\n# D\n".to_string(),
        )]
    }

    fn placeset(
        subject: (&str, &str),
        reference: (&str, &str),
        directions: Vec<Direction>,
    ) -> Op {
        Op::PlaceSet {
            diagram: "dia".into(),
            subject_title: subject.0.into(),
            subject_slug: subject.1.into(),
            reference_title: reference.0.into(),
            reference_slug: reference.1.into(),
            directions,
        }
    }

    #[test]
    fn place_set_adds_a_left_of_placement() {
        let b = layout_diagram("- [Customer](./customer.md) below [Order](./order.md)\n");
        let out = apply(
            &b,
            &[placeset(
                ("Order", "order"),
                ("PaymentGateway", "payment-gateway"),
                vec![Direction::LeftOf],
            )],
        )
        .unwrap();
        assert!(
            out[0]
                .1
                .contains("- [Order](./order.md) left of [PaymentGateway](./payment-gateway.md)"),
            "authored placement present: {}",
            out[0].1
        );
        assert!(
            out[0]
                .1
                .contains("- [Customer](./customer.md) below [Order](./order.md)"),
            "existing layout line kept: {}",
            out[0].1
        );
    }

    #[test]
    fn place_set_creates_layout_section_when_absent() {
        let out = apply(
            &diagram_no_layout(),
            &[placeset(
                ("Order", "order"),
                ("PaymentGateway", "payment-gateway"),
                vec![Direction::LeftOf],
            )],
        )
        .unwrap();
        assert!(
            out[0].1.contains("## Layout"),
            "Layout section created when absent: {}",
            out[0].1
        );
        assert!(out[0]
            .1
            .contains("- [Order](./order.md) left of [PaymentGateway](./payment-gateway.md)"));
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p waml place_set_adds_a_left_of_placement place_set_creates_layout_section_when_absent`
Expected: FAIL — compile error `no variant named PlaceSet found for enum Op` (the variant, helpers, and dispatch do not exist yet).

- [ ] **Step 3: Extend the syntax import**

In `crates/waml/src/ops/mod.rs`, replace the import at line 7:

```rust
use crate::syntax::{Document, Line, ParsedName, ParsedRel, Section};
```

with:

```rust
use crate::syntax::{
    Direction, Document, LayoutItem, LayoutStatement, Line, NameRef, Operand, OperandRef,
    ParsedName, ParsedRel, Section,
};
```

- [ ] **Step 4: Add the `Op::PlaceSet` variant**

In the `Op` enum, immediately after the `DiagramSet { .. }` variant's closing `},` (before the enum's closing `}` at `:165`), add:

```rust
    PlaceSet {
        diagram: String,
        subject_title: String,
        subject_slug: String,
        reference_title: String,
        reference_slug: String,
        directions: Vec<Direction>,
    },
```

- [ ] **Step 5: Add the `apply_one` dispatch arm**

In `apply_one`, after the `Op::DiagramSet { .. } => op_diagram_set(...)` arm (before the match's closing `}` at `:264`), add:

```rust
        Op::PlaceSet {
            diagram,
            subject_title,
            subject_slug,
            reference_title,
            reference_slug,
            directions,
        } => op_place_set(
            work,
            diagram,
            subject_title,
            subject_slug,
            reference_title,
            reference_slug,
            directions,
        ),
```

- [ ] **Step 6: Add the `layout_mut` helper**

Immediately after `attrs_mut` (ends at `:336`), add:

```rust
/// Get the `## Layout` list, creating an empty section if absent
/// (canonical serialize re-orders sections, so append position is irrelevant).
pub(crate) fn layout_mut(doc: &mut Document) -> &mut Vec<Line<LayoutItem>> {
    if !doc.sections.iter().any(|s| matches!(s, Section::Layout(_))) {
        doc.sections.push(Section::Layout(Vec::new()));
    }
    doc.sections
        .iter_mut()
        .find_map(|s| match s {
            Section::Layout(l) => Some(l),
            _ => None,
        })
        .expect("layout section just ensured")
}
```

- [ ] **Step 7: Add `link_operand` and the base `op_place_set`**

Add these free functions (e.g. just before `op_node_rm` at `:945`, or anywhere at module scope):

```rust
/// A `[title](./slug.md)` operand with no axis/hints.
fn link_operand(title: &str, slug: &str) -> Operand {
    Operand {
        ref_: OperandRef::Name(NameRef::Link {
            title: title.to_string(),
            slug: slug.to_string(),
        }),
        axis: None,
        hints: vec![],
    }
}

fn op_place_set(
    work: &mut Bundle,
    diagram: &str,
    subject_title: &str,
    subject_slug: &str,
    reference_title: &str,
    reference_slug: &str,
    directions: &[Direction],
) -> Result<(), OpError> {
    let subject_title = subject_title.to_string();
    let subject_slug = subject_slug.to_string();
    let reference_title = reference_title.to_string();
    let reference_slug = reference_slug.to_string();
    let dir = directions[0];
    edit_doc(work, diagram, "place.set", |doc| {
        let layout = layout_mut(doc);
        let stmt = LayoutStatement::Placement {
            operands: vec![
                link_operand(&subject_title, &subject_slug),
                link_operand(&reference_title, &reference_slug),
            ],
            directions: vec![dir],
        };
        layout.push(Line::Parsed(LayoutItem { line: 0, stmt }));
        Ok(())
    })
}
```

- [ ] **Step 8: Run the tests to verify they pass**

Run: `cargo test -p waml place_set_adds_a_left_of_placement place_set_creates_layout_section_when_absent`
Expected: PASS (both tests).

- [ ] **Step 9: Commit**

```bash
git add crates/waml/src/ops/mod.rs
git commit -m "feat(ops): Op::PlaceSet authors a ## Layout placement (base)"
```

---

### Task 2: Same-axis replace

Authoring a placement between the same subject/reference pair replaces a prior placement (regardless of axis for now — narrowed in Task 3). Introduces the `retain` step plus `operand_slug`/`placement_matches` helpers.

**Files:**
- Modify: `crates/waml/src/ops/mod.rs` (`op_place_set`; new free helpers `operand_slug`, `placement_matches`)
- Test: `crates/waml/src/ops/mod.rs` (`#[cfg(test)] mod tests`)

**Interfaces:**
- Consumes: `Operand`, `OperandRef`, `NameRef`, `LayoutStatement` (already imported in Task 1); `link_operand`, `layout_mut` (from Task 1).
- Produces:
  - `fn operand_slug(op: &Operand) -> Option<&str>`
  - `fn placement_matches(stmt: &LayoutStatement, subject: &str, reference: &str) -> bool`

- [ ] **Step 1: Write the failing test**

Add to `mod tests`:

```rust
    #[test]
    fn place_set_replaces_same_axis_placement() {
        let b = layout_diagram(
            "- [Order](./order.md) left of [PaymentGateway](./payment-gateway.md)\n",
        );
        let out = apply(
            &b,
            &[placeset(
                ("Order", "order"),
                ("PaymentGateway", "payment-gateway"),
                vec![Direction::RightOf],
            )],
        )
        .unwrap();
        assert!(
            out[0]
                .1
                .contains("- [Order](./order.md) right of [PaymentGateway](./payment-gateway.md)"),
            "new horizontal placement present: {}",
            out[0].1
        );
        assert!(
            !out[0].1.contains("left of"),
            "prior same-pair placement replaced, not duplicated: {}",
            out[0].1
        );
    }
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p waml place_set_replaces_same_axis_placement`
Expected: FAIL — the assertion `!out[0].1.contains("left of")` fails; the base `op_place_set` only pushes, so both `left of` and `right of` are present.

- [ ] **Step 3: Add `operand_slug` and `placement_matches`**

Add at module scope (next to `link_operand`):

```rust
/// The bare slug an operand references (Link href stem or bare name), if any.
fn operand_slug(op: &Operand) -> Option<&str> {
    match &op.ref_ {
        OperandRef::Name(NameRef::Link { slug, .. }) => Some(slug.as_str()),
        OperandRef::Name(NameRef::Bare(s)) => Some(s.as_str()),
        _ => None,
    }
}

/// A 2-operand `[subject] <dir> [reference]` placement (any direction).
fn placement_matches(stmt: &LayoutStatement, subject: &str, reference: &str) -> bool {
    let LayoutStatement::Placement {
        operands,
        directions,
    } = stmt
    else {
        return false;
    };
    operands.len() == 2
        && directions.len() == 1
        && operand_slug(&operands[0]) == Some(subject)
        && operand_slug(&operands[1]) == Some(reference)
}
```

- [ ] **Step 4: Add the `retain` step to `op_place_set`**

In `op_place_set`, inside the `edit_doc` closure, insert the `retain` between `let layout = layout_mut(doc);` and the `let stmt = ...` push:

```rust
    edit_doc(work, diagram, "place.set", |doc| {
        let layout = layout_mut(doc);
        layout.retain(|line| match line.parsed() {
            Some(item) => !placement_matches(&item.stmt, &subject_slug, &reference_slug),
            None => true,
        });
        let stmt = LayoutStatement::Placement {
            operands: vec![
                link_operand(&subject_title, &subject_slug),
                link_operand(&reference_title, &reference_slug),
            ],
            directions: vec![dir],
        };
        layout.push(Line::Parsed(LayoutItem { line: 0, stmt }));
        Ok(())
    })
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test -p waml place_set`
Expected: PASS (all three `place_set_*` tests so far).

- [ ] **Step 6: Commit**

```bash
git add crates/waml/src/ops/mod.rs
git commit -m "feat(ops): PlaceSet replaces a prior same-pair placement"
```

---

### Task 3: Axis-scoped replace (different axis is kept)

Narrow the replace to the **same axis only**: a new horizontal placement must not drop an existing vertical one between the same pair.

**Files:**
- Modify: `crates/waml/src/ops/mod.rs` (`op_place_set`; `placement_matches`; new `dir_is_horizontal`)
- Test: `crates/waml/src/ops/mod.rs` (`#[cfg(test)] mod tests`)

**Interfaces:**
- Consumes: `Direction` (imported in Task 1); `placement_matches` (from Task 2, signature changes here).
- Produces:
  - `fn dir_is_horizontal(d: Direction) -> bool`
  - `placement_matches(stmt: &LayoutStatement, subject: &str, reference: &str, horizontal: bool) -> bool` (adds the `horizontal` param)

- [ ] **Step 1: Write the failing test**

Add to `mod tests`:

```rust
    #[test]
    fn place_set_keeps_placement_on_a_different_axis() {
        let b = layout_diagram(
            "- [Order](./order.md) above [PaymentGateway](./payment-gateway.md)\n",
        );
        let out = apply(
            &b,
            &[placeset(
                ("Order", "order"),
                ("PaymentGateway", "payment-gateway"),
                vec![Direction::LeftOf],
            )],
        )
        .unwrap();
        assert!(
            out[0].1.contains("left of"),
            "new horizontal placement added: {}",
            out[0].1
        );
        assert!(
            out[0].1.contains("above"),
            "vertical placement on the other axis is untouched: {}",
            out[0].1
        );
    }
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p waml place_set_keeps_placement_on_a_different_axis`
Expected: FAIL — the assertion `out[0].1.contains("above")` fails; Task 2's axis-blind `placement_matches` drops the vertical `above` statement before pushing the horizontal one.

- [ ] **Step 3: Add `dir_is_horizontal`**

Add at module scope (next to `operand_slug`):

```rust
/// Horizontal axis = Left/Right; Vertical = Above/Below.
fn dir_is_horizontal(d: Direction) -> bool {
    matches!(d, Direction::LeftOf | Direction::RightOf)
}
```

- [ ] **Step 4: Add the axis check to `placement_matches`**

Replace `placement_matches` with the axis-aware version:

```rust
/// A 2-operand `[subject] <dir> [reference]` placement on the given axis.
fn placement_matches(
    stmt: &LayoutStatement,
    subject: &str,
    reference: &str,
    horizontal: bool,
) -> bool {
    let LayoutStatement::Placement {
        operands,
        directions,
    } = stmt
    else {
        return false;
    };
    operands.len() == 2
        && directions.len() == 1
        && dir_is_horizontal(directions[0]) == horizontal
        && operand_slug(&operands[0]) == Some(subject)
        && operand_slug(&operands[1]) == Some(reference)
}
```

- [ ] **Step 5: Pass the axis into the `retain` call**

In `op_place_set`, compute the axis from `dir` and pass it. Update the closure so the `retain` reads:

```rust
    edit_doc(work, diagram, "place.set", |doc| {
        let layout = layout_mut(doc);
        let horizontal = dir_is_horizontal(dir);
        layout.retain(|line| match line.parsed() {
            Some(item) => {
                !placement_matches(&item.stmt, &subject_slug, &reference_slug, horizontal)
            }
            None => true,
        });
        let stmt = LayoutStatement::Placement {
            operands: vec![
                link_operand(&subject_title, &subject_slug),
                link_operand(&reference_title, &reference_slug),
            ],
            directions: vec![dir],
        };
        layout.push(Line::Parsed(LayoutItem { line: 0, stmt }));
        Ok(())
    })
```

- [ ] **Step 6: Run the tests to verify they pass**

Run: `cargo test -p waml place_set`
Expected: PASS (all four `place_set_*` tests).

- [ ] **Step 7: Commit**

```bash
git add crates/waml/src/ops/mod.rs
git commit -m "feat(ops): PlaceSet replace is axis-scoped (keeps the other axis)"
```

---

### Task 4: Corner drop — 1 or 2 directions author 1 or 2 statements

Iterate **all** directions (not just `directions[0]`), applying axis-scoped replace per direction. A corner drop (`[LeftOf, Above]`) authors two separate 2-operand statements.

**Files:**
- Modify: `crates/waml/src/ops/mod.rs` (`op_place_set`)
- Test: `crates/waml/src/ops/mod.rs` (`#[cfg(test)] mod tests`)

**Interfaces:**
- Consumes: `layout_mut`, `link_operand`, `dir_is_horizontal`, `placement_matches` (from Tasks 1-3).
- Produces: final `op_place_set` looping over `directions`.

- [ ] **Step 1: Write the failing test**

Add to `mod tests` (uses `parse_document` and `Section`, reachable via `use super::*;`):

```rust
    #[test]
    fn place_set_corner_authors_two_statements() {
        let out = apply(
            &diagram_no_layout(),
            &[placeset(
                ("Order", "order"),
                ("PaymentGateway", "payment-gateway"),
                vec![Direction::LeftOf, Direction::Above],
            )],
        )
        .unwrap();
        assert!(out[0].1.contains("left of"), "horizontal statement: {}", out[0].1);
        assert!(out[0].1.contains("above"), "vertical statement: {}", out[0].1);
        // Two separate 2-operand placement bullets
        // (invariant: directions.len() == operands.len() - 1).
        let doc = parse_document(&out[0].1);
        let layout = doc
            .sections
            .iter()
            .find_map(|s| match s {
                Section::Layout(l) => Some(l),
                _ => None,
            })
            .expect("layout section present");
        assert_eq!(layout.len(), 2, "corner drop authored two statements");
    }
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p waml place_set_corner_authors_two_statements`
Expected: FAIL — `assert_eq!(layout.len(), 2)` fails with `1`; the current `op_place_set` only handles `directions[0]`, so only the `left of` statement is authored.

- [ ] **Step 3: Loop over all directions in `op_place_set`**

Replace the body of `op_place_set` so it iterates every direction. The final function reads:

```rust
fn op_place_set(
    work: &mut Bundle,
    diagram: &str,
    subject_title: &str,
    subject_slug: &str,
    reference_title: &str,
    reference_slug: &str,
    directions: &[Direction],
) -> Result<(), OpError> {
    let subject_title = subject_title.to_string();
    let subject_slug = subject_slug.to_string();
    let reference_title = reference_title.to_string();
    let reference_slug = reference_slug.to_string();
    let directions = directions.to_vec();
    edit_doc(work, diagram, "place.set", |doc| {
        let layout = layout_mut(doc);
        for dir in &directions {
            let horizontal = dir_is_horizontal(*dir);
            layout.retain(|line| match line.parsed() {
                Some(item) => {
                    !placement_matches(&item.stmt, &subject_slug, &reference_slug, horizontal)
                }
                None => true,
            });
            let stmt = LayoutStatement::Placement {
                operands: vec![
                    link_operand(&subject_title, &subject_slug),
                    link_operand(&reference_title, &reference_slug),
                ],
                directions: vec![*dir],
            };
            layout.push(Line::Parsed(LayoutItem { line: 0, stmt }));
        }
        Ok(())
    })
}
```

- [ ] **Step 4: Run the whole waml suite to verify it passes**

Run: `cargo test -p waml`
Expected: PASS (all `place_set_*` tests plus the full existing suite; the earlier single-direction tests still pass because the loop degenerates to one iteration).

- [ ] **Step 5: Commit**

```bash
git add crates/waml/src/ops/mod.rs
git commit -m "feat(ops): PlaceSet authors one statement per direction (corner = 2)"
```

---

### Task 5: `waml-ops-dto` exhaustive-match arm

Adding `Op::PlaceSet` makes the exhaustive `from_op` match in `waml-ops-dto` non-exhaustive, breaking that crate's compile. `PlaceSet` has no web DTO in Phase A (native-only), so the arm is an explicit `unreachable!`.

**Files:**
- Modify: `crates/waml-ops-dto/src/lib.rs` (`from_op` match, before its closing brace at `:765`)

**Interfaces:**
- Consumes: `Op::PlaceSet` (from Task 1).
- Produces: an `unreachable!` arm; `waml-ops-dto` compiles again.

- [ ] **Step 1: Confirm the crate currently fails to compile**

Run: `cargo build -p waml-ops-dto`
Expected: FAIL — `error[E0004]: non-exhaustive patterns: '&PlaceSet { .. }' not covered` in `from_op`.

- [ ] **Step 2: Add the `unreachable!` arm**

In `crates/waml-ops-dto/src/lib.rs`, inside the `from_op` `match op { ... }`, immediately after the `Op::DiagramSet { .. } => OpDto::DiagramSet { .. },` arm (its closing `},` around `:764`) and before the match's closing `}`, add:

```rust
            Op::PlaceSet { .. } => {
                unreachable!("place.set no web DTO yet (native-only)")
            }
```

- [ ] **Step 3: Run the build to verify it passes**

Run: `cargo build -p waml-ops-dto`
Expected: PASS (compiles clean).

- [ ] **Step 4: Run both crates' tests to confirm nothing regressed**

Run: `cargo test -p waml && cargo test -p waml-ops-dto`
Expected: PASS (both suites). The `waml-ops-dto` `from_op` round-trip test (`:809`) has a `_ => panic!` catch-all and needs no change.

- [ ] **Step 5: Commit**

```bash
git add crates/waml-ops-dto/src/lib.rs
git commit -m "feat(ops-dto): exhaustive from_op arm for Op::PlaceSet (native-only)"
```

---

## Self-Review

**1. Spec coverage** — every required behavior and test maps to a task:
- 1-or-2 directions → separate statements, invariant `directions.len() == operands.len() - 1`: Task 4 (loop, corner test asserts 2 statements).
- Operands authored as `NameRef::Link { title, slug }` from the op's subject/reference fields: Task 1 (`link_operand`).
- Axis-scoped replace-only-same-reference (Horizontal = Left/Right, Vertical = Above/Below): Task 2 (replace) + Task 3 (axis scoping).
- In-memory only (through `edit_doc`, no disk write): Tasks 1-4 (all mutation is inside `edit_doc`).
- Test: add `left of` to a `## Layout`: Task 1.
- Test: same-axis REPLACE (`left of` → `right of`): Task 2.
- Test: different-axis KEEP (`above` kept when adding `left of`): Task 3.
- Test: corner (2 dirs) → two statements: Task 4.
- Test: Layout section created when absent: Task 1 (`place_set_creates_layout_section_when_absent`, via `layout_mut`).
- Test: `waml-ops-dto` still compiles (unreachable arm): Task 5.

**2. Placeholder scan** — no TBD/TODO/"add error handling"/"similar to Task N"; every code step shows the full code and every command has an expected outcome.

**3. Type consistency** — `placement_matches` is introduced in Task 2 with `(stmt, subject, reference)` and deliberately re-signed in Task 3 to add `horizontal: bool`; the `op_place_set` call site is updated in the same task (Task 3, Step 5). `dir_is_horizontal`, `operand_slug`, `link_operand`, `layout_mut` keep stable signatures across tasks. The `Op::PlaceSet` field set (`diagram`, `subject_title`, `subject_slug`, `reference_title`, `reference_slug`, `directions`) is identical in the enum (Task 1, Step 4), the `apply_one` arm (Task 1, Step 5), the `placeset` test helper (Task 1, Step 1), and the `from_op` arm (Task 5, Step 2, via `{ .. }`).
