# Instances + Object Diagrams Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.
> **Gate:** full
> **Rigor:** tdd-per-task

**Goal:** Land the whole data model + conformance validation for UML `InstanceSpecification` (objects) and links (object-diagram edges) — model, grammar, generated bindings, and warn-only validation — so a later object-diagram renderer and slice 4 (instances-as-lifelines) build on a stable shape. **No renderer.**

**Architecture:** Objects are one more element kind in the same pool as classes (design spec §5). A standalone `type: uml.InstanceSpecification` doc, and an inline `## Members` object in a Diagram, both produce a pool `InstanceSpecification` `Node` carrying `slots: Vec<Slot>`. Its classifier is an `InstanceOf` edge; an object-to-object link is a `Links` edge — both ordinary `Model.edges`. New controlled-prose productions round-trip byte-identically. Three warn-only diagnostics check conformance. The Rust runtime shape is emitted to TypeScript through Tsify.

**Tech Stack:** Rust (`waml` crate; `cargo`, serde, `tsify_next`/`wasm-bindgen`, `regex`), TypeScript monorepo (`packages/`, `pnpm`, Vitest).

**Slug:** `instances-object-diagrams`.

This document preserves the fully-elaborated plan verbatim. The `README.md` is the shorter overview; each `### Task N` section below is also carried as its own authoritative `task-N-*.md` file (the implement-plan directory workflow reads those per-task files).

## Predecessors landed (assume all on `main`)

Slice 1 (element-pool rename + `is_classifier()`), slice 2 (behavior model/view split — `ElementType::is_view()`, `ActivityNode`/`FlowEdge` pools), and the sequence-flat-model slice are all on `main`. **Locate every edit site by struct/enum/function NAME, not by line number.** This slice does NOT touch the flow pools, the sequence types, `ElementType::is_view()`, or `Lifeline.ref_`.

## Global Constraints

- **Full CI gate, in this order:** `cargo fmt --check` → `cargo clippy` (no new warnings) → `cargo test --workspace` → `pnpm build:wasm` → `pnpm lint` → `pnpm build` → `pnpm -r test`. Every task leaves the tree green under the full gate before its commit.
- **`is_classifier()` is explicit — no `_ =>` catch-all.** `InstanceSpecification` → `false` is an added arm. `is_view()` stays `false` for it.
- **`Slot` mirrors `Attribute`:** serde `ref` (skip-if-none) + tsify. `Node.slots` is skip-if-empty (mirrors `values`).
- **`RelationshipKind::{InstanceOf, Links}` are ordinary edges in `Model.edges`** — no separate pool/type. `Links` reuses `Edge.name: Option<AssocName>`; neither takes ends.
- **Controlled-prose reserved tokens:** `instance of`, `with`, `set to`, `and`, `as`, `links`. A slot value is a quoted string, a bare ident/number, or a `[Label](./ref.md)` link. **Canonical serialize joins slots with ` and `** for byte-identical round-trip.
- **Validation is warn-only** (three codes) — object diagrams are tolerant, like `Unknown` types (OKF §9).
- **Out of scope (guards):** NO renderer. Do NOT touch `Lifeline.ref_` (classifier-only; instances-as-lifelines is slice §7.4). Inline is **instances-only**. Do NOT redesign or break round-trip of any untouched syntax layer. Do NOT modify the TS `RELATIONSHIP_KINDS` / `ENDED_KINDS` lists or `packages/okf/src/grammar.ts`. Do NOT hand-edit the generated `packages/wasm/src/generated/*` files.

## File Structure

Rust (`crates/waml/src/`): `model.rs`, `syntax.rs`, `grammar.rs`, `parse.rs`, `serialize.rs`, `validate.rs`, `diagnostic.rs`; tests in `crates/waml/tests/serde_shape.rs` + inline `#[cfg(test)]`. Regenerated binding (Tasks 2 & 6, do NOT hand-edit): `packages/wasm/src/generated/waml_wasm.d.ts` (+ `.js`). TS: `packages/wasm/src/index.ts`, `packages/okf/src/types.ts`, `packages/core/src/state/overlay.ts`.

Six ordered tasks. Each is independently green under the full gate; Task 2 and Task 6 each bundle a wasm-binding regen with their Rust change in ONE commit (the coupling seam). Tasks 3/4/5 change no tsify-visible type and assert the binding is unchanged.

---

### Task 1: InstanceSpecification metaclass + is_classifier

Add the `InstanceSpecification` UML metaclass and pin its classifier semantics. **Rust-only, self-contained** — `UmlMetaclass` has no serde/tsify derive and `ElementType` serializes to a bare string, so **no wasm binding regenerates**. Independently green under `cargo test -p waml`.

**Guardrails:** `is_classifier()` returns `false` for `InstanceSpecification` (spec §3.1), written as an explicit arm (extend the existing `Package | Note => false` arm); the `Uml(mc)` match has no `_ =>` catch-all. `is_view()` stays `false` (pin with a test).

**Files:** `crates/waml/src/model.rs` — `enum UmlMetaclass`, `UmlMetaclass::parse`, `UmlMetaclass::name`, `ElementType::is_classifier`, and the two tests `is_classifier_matches_spec_table` / `is_view_flags_diagrams_and_behaviors`.

**Interfaces — Produces:** `UmlMetaclass::InstanceSpecification`; `ElementType::parse("uml.InstanceSpecification") == ElementType::Uml(UmlMetaclass::InstanceSpecification)`, `.as_str() == "uml.InstanceSpecification"`; `is_classifier() == false`, `is_view() == false`.

- [ ] **Step 1.1: Write the failing tests.** Add a new round-trip test and extend the two existing predicate tests:

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
        assert!(!ElementType::Uml(UmlMetaclass::InstanceSpecification).is_classifier());
        assert!(!ElementType::Uml(UmlMetaclass::InstanceSpecification).is_view());
    }
```

  Add `assert!(!ElementType::Uml(UmlMetaclass::InstanceSpecification).is_classifier());` inside `is_classifier_matches_spec_table` and `assert!(!ElementType::Uml(UmlMetaclass::InstanceSpecification).is_view());` inside `is_view_flags_diagrams_and_behaviors`.

- [ ] **Step 1.2: Run, verify fail to compile.** `cargo test -p waml --lib instance_specification` — FAIL: variant absent.

- [ ] **Step 1.3: Add the variant + parse/name arms.** In `pub enum UmlMetaclass` add `InstanceSpecification` after `UseCase`. In `parse` add `"InstanceSpecification" => Some(UmlMetaclass::InstanceSpecification),`. In `name` (exhaustive) add `UmlMetaclass::InstanceSpecification => "InstanceSpecification",`.

- [ ] **Step 1.4: Extend the `is_classifier` arm** to `UmlMetaclass::Package | UmlMetaclass::Note | UmlMetaclass::InstanceSpecification => false`. No `_ =>` catch-all.

- [ ] **Step 1.5: Check other exhaustive matches.** `grep -rn "UmlMetaclass::" crates/waml/src crates/waml-editor/src` — only `model.rs` matches exhaustively; `validate.rs`'s `matches!(Actor | UseCase)` needs no arm (do NOT widen it).

- [ ] **Step 1.6: Green.** `cargo test -p waml && cargo clippy -p waml --all-targets && cargo fmt`.

- [ ] **Step 1.7: Commit.**
  ```
  git add crates/waml/src/model.rs
  git commit -F - <<'EOF'
  feat(model): add InstanceSpecification metaclass (non-classifier)

  Plan: instances-object-diagrams
  Plan-Tasks: Task 1
  EOF
  ```

---

### Task 2: Slot, Node.slots, and InstanceOf/Links edge kinds (wire shape + binding + TS)

Add every tsify-visible runtime type this slice needs in ONE green commit: the runtime `Slot`, the `Node.slots` pool, and the two new `RelationshipKind` variants, then regenerate the binding and update the TS barrels + `ModelGraph` mapping. The checked-in `waml_wasm.d.ts` couples the Rust shape to its TS consumers, so a Rust-only intermediate fails `pnpm build`. Nothing populates `slots` or emits the new edges yet (Tasks 3–5); this fixes the SHAPE only.

**Guardrails:** `Slot` mirrors `Attribute` (serde `ref` skip-if-none; tsify). `Node.slots: Vec<Slot>` skip-if-empty (mirrors `values`). The classifier ref is NOT a `Node` field. `RelationshipKind::{InstanceOf, Links}` are ordinary edge kinds; `is_ended() == false`; `Links` uses the existing `name: Option<AssocName>`. Do NOT modify `RELATIONSHIP_KINDS` / `ENDED_KINDS` or `packages/okf/src/grammar.ts`. Do NOT hand-edit generated files.

**Files:** `crates/waml/src/model.rs`, `crates/waml/src/parse.rs` (Node literals), `crates/waml/tests/serde_shape.rs`, `packages/wasm/src/generated/waml_wasm.d.ts` (+ `.js`, regenerated), `packages/wasm/src/index.ts`, `packages/okf/src/types.ts`, `packages/core/src/state/overlay.ts`.

**Interfaces — Produces (Rust):** `pub struct Slot { pub name: String, pub value: String, pub ref_: Option<String> }`; `Node.slots: Vec<Slot>`; `RelationshipKind::InstanceOf` (`as_str "instance of"`, wire `"instanceof"`), `RelationshipKind::Links` (`as_str "links"`, wire `"links"`), both `is_ended() == false`. **Produces (TS):** `Slot = { name; value; ref? }`; `Node.slots?: Slot[]`; `RelationshipKind` gains `"instanceof" | "links"`; `ModelNode.slots?: Slot[]`.

#### Phase A — Rust runtime shape (TDD)

- [ ] **Step 2.1: Write the failing wire-shape tests** in `crates/waml/tests/serde_shape.rs` (import `Slot`, `RelationshipKind`):

```rust
#[test]
fn slot_serializes_with_ref_key_and_skips_none() {
    use waml::model::Slot;
    let bare = Slot { name: "id".into(), value: "ORD-42".into(), ref_: None };
    let v = serde_json::to_value(&bare).unwrap();
    assert_eq!(v["name"], "id");
    assert_eq!(v["value"], "ORD-42");
    assert!(v.get("ref").is_none(), "None ref must be omitted: {v}");
    let linked = Slot { name: "customer".into(), value: "Ann".into(), ref_: Some("m/ann".into()) };
    assert_eq!(serde_json::to_value(&linked).unwrap()["ref"], "m/ann");
}

#[test]
fn instance_edge_kinds_serialize_lowercase() {
    use waml::model::RelationshipKind;
    assert_eq!(serde_json::to_value(RelationshipKind::InstanceOf).unwrap(), serde_json::json!("instanceof"));
    assert_eq!(serde_json::to_value(RelationshipKind::Links).unwrap(), serde_json::json!("links"));
    assert_eq!(RelationshipKind::InstanceOf.as_str(), "instance of");
    assert_eq!(RelationshipKind::Links.as_str(), "links");
    assert!(!RelationshipKind::InstanceOf.is_ended());
    assert!(!RelationshipKind::Links.is_ended());
}

#[test]
fn classifier_node_omits_empty_slots() {
    let m = build_model(&bundle());
    let v = serde_json::to_value(&m).unwrap();
    assert!(v["nodes"][0].get("slots").is_none(), "empty slots must be omitted: {}", v["nodes"][0]);
}
```

- [ ] **Step 2.2: Run, verify fail to compile.** `cargo test -p waml --features serde --test serde_shape`.

- [ ] **Step 2.3: Add the `Slot` struct** in `model.rs`, immediately after `Attribute`:

```rust
/// A slot value on an `InstanceSpecification` (design spec §3.2): a named value
/// that stands in for a classifier attribute. Mirrors `Attribute` for serde/tsify.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "wasm", derive(tsify_next::Tsify))]
#[cfg_attr(feature = "wasm", tsify(into_wasm_abi, from_wasm_abi))]
pub struct Slot {
    pub name: String,
    pub value: String,
    #[cfg_attr(
        feature = "serde",
        serde(rename = "ref", default, skip_serializing_if = "Option::is_none")
    )]
    pub ref_: Option<String>,
}
```

- [ ] **Step 2.4: Add `Node.slots`** after `members`:

```rust
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Vec::is_empty")
    )]
    pub slots: Vec<Slot>,
```

- [ ] **Step 2.5: Add the `RelationshipKind` variants.** In the enum add `InstanceOf,` and `Links,` after `Extends`. `as_str`: `InstanceOf => "instance of"`, `Links => "links"`. `parse`: `"instance of" => Some(RelationshipKind::InstanceOf)`, `"links" => Some(RelationshipKind::Links)`. Leave `is_ended()` unchanged (both already `false`).

- [ ] **Step 2.6: Extend `relationship_kind_round_trips`** (model.rs test) with `RelationshipKind::InstanceOf,` and `RelationshipKind::Links,`.

- [ ] **Step 2.7: Add `slots` to every `Node { … }` literal** — `build_node` and `build_packages` (`parse.rs`, `slots: Vec::new()` / `slots: vec![]`), the two literals in `serde_shape.rs` (`package_node_and_model_path`), and the one in `model.rs` tests. `grep -rn "Node {" crates/waml/src crates/waml/tests` — 5 total; do NOT touch `ErrorNode`/`ActivityNode`/`SeqNode`/`SceneNode`.

- [ ] **Step 2.8: Green (Rust).** `cargo test -p waml --features serde --test serde_shape && cargo test --workspace && cargo clippy -p waml --all-targets && cargo fmt`.

#### Phase B — Regenerate the binding

- [ ] **Step 2.9: Regenerate.** `pnpm build:wasm` then `git diff packages/wasm/src/generated/waml_wasm.d.ts`. Expect `export interface Slot { name; value; ref? }`, `Node.slots?: Slot[]`, and `RelationshipKind = … | "instanceof" | "links"`. Do NOT hand-edit.

#### Phase C — TS barrels + ModelGraph

- [ ] **Step 2.10: `@waml/wasm` barrel.** In `packages/wasm/src/index.ts` add `Slot,` next to `Attribute,` in the `export type { … } from "./generated/waml_wasm.js";` block.

- [ ] **Step 2.11: `@waml/okf` barrel + ModelNode.** In `packages/okf/src/types.ts` add `Slot,` to both the `export type { … }` and `import type { … }` blocks, and add to `interface ModelNode` after `attributes`:
  ```ts
      /** Slot values on a uml.InstanceSpecification node (design spec §3.3). Absent on non-instances. */
      slots?: Slot[];
  ```

- [ ] **Step 2.12: Map `slots` in `toModelGraph`.** In `packages/core/src/state/overlay.ts` `toNode`, after `attributes`:
  ```ts
      ...(n.slots && n.slots.length ? { slots: n.slots } : {}),
  ```

#### Phase D — Full gate + commit

- [ ] **Step 2.13: Green (TS).** `pnpm lint && pnpm build && pnpm -r test` — including `templates.test.ts` (unchanged `RELATIONSHIP_KINDS` is fine; templates carry no instance edges).

- [ ] **Step 2.14: Commit.**
  ```
  git add crates/waml/src/model.rs crates/waml/src/parse.rs crates/waml/tests/serde_shape.rs packages/wasm/src/generated/waml_wasm.d.ts packages/wasm/src/generated/waml_wasm.js packages/wasm/src/index.ts packages/okf/src/types.ts packages/core/src/state/overlay.ts
  git commit -F - <<'EOF'
  feat(model): add Slot, Node.slots, and instance-of/links edge kinds

  Plan: instances-object-diagrams
  Plan-Tasks: Task 2
  EOF
  ```
  (Add any other regenerated `packages/wasm/src/generated/*` per `git status`.)

---

### Task 3: Standalone instance Slots section (## Slots parse/serialize + build)

Wire `## Slots` end-to-end. A standalone `type: uml.InstanceSpecification` doc with a `## Slots` list produces a pool `Node` with populated `slots`. **Rust-only, no binding change** (`Slot`/`Node.slots` landed in Task 2) — assert `git diff --exit-code` on the binding.

**Guardrails:** value = quoted string | bare ident/number | `[Label](./ref.md)` link. **Byte-identical round-trip** — quoting is part of the surface form; preserve it in the syntax layer (`SlotValue`), distinct from the resolved `Slot`. Additive production only.

**Files:** `syntax.rs`, `grammar.rs`, `parse.rs`, `serialize.rs`, tests in `grammar.rs`/`serde_shape.rs`/`serialize.rs`.

**Interfaces — Produces:** `enum SlotValue { Quoted(String), Bare(String), Link(LinkRef) }`; `struct ParsedSlot { name, value, line, span }`; `Section::Slots(Vec<Line<ParsedSlot>>)`; `parse_slot_line` / `render_slot_line` (exact inverse).

#### Phase A — syntax + grammar (TDD)

- [ ] **Step 3.1: Write the failing grammar round-trip test** in `grammar.rs`:

```rust
    #[test]
    fn slot_lines_round_trip_all_three_value_forms() {
        for line in ["- id: \"ORD-42\"", "- status: PLACED", "- qty: 3", "- customer: [Ann](./ann.md)"] {
            let s = parse_slot_line(line).unwrap();
            assert_eq!(render_slot_line(&s), line, "slot line must round-trip byte-identically");
        }
    }

    #[test]
    fn slot_value_classifies_quoted_bare_and_link() {
        use crate::syntax::SlotValue;
        assert!(matches!(parse_slot_line("- id: \"ORD-42\"").unwrap().value, SlotValue::Quoted(v) if v == "ORD-42"));
        assert!(matches!(parse_slot_line("- status: PLACED").unwrap().value, SlotValue::Bare(v) if v == "PLACED"));
        let SlotValue::Link(l) = parse_slot_line("- customer: [Ann](./ann.md)").unwrap().value else { panic!() };
        assert_eq!((l.title.as_str(), l.slug.as_str()), ("Ann", "ann"));
    }
```

- [ ] **Step 3.2: Run, verify fail to compile.** `cargo test -p waml --lib slot_`.

- [ ] **Step 3.3: Add the syntax types** in `syntax.rs` near `ParsedRel`:

```rust
/// A `## Slots` value's SURFACE form (preserved for byte-identical round-trip),
/// distinct from the resolved `model::Slot`.
#[derive(Debug, Clone, PartialEq)]
pub enum SlotValue {
    Quoted(String),
    Bare(String),
    Link(LinkRef),
}

/// One `## Slots` bullet: `- name: value`.
#[derive(Debug, Clone, PartialEq)]
pub struct ParsedSlot {
    pub name: String,
    pub value: SlotValue,
    pub line: usize,
    pub span: Option<(usize, usize)>,
}
```

  Add `Slots(Vec<Line<ParsedSlot>>),` to `pub enum Section`.

- [ ] **Step 3.4: Add `parse_slot_line` / `render_slot_line`** in `grammar.rs` (import `ParsedSlot`, `SlotValue`):

```rust
static SLOT_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^- ([A-Za-z_][A-Za-z0-9_]*): (.+)$").unwrap());

pub fn parse_slot_line(line: &str) -> Result<ParsedSlot, LineError> {
    let err = || LineError {
        range: bullet_range(line),
        message: "malformed slot — expected '- name: value' (value = \"quoted\", bare token, or [Label](./slug.md))".to_string(),
    };
    let trimmed = line.trim_end_matches('\r').trim();
    let caps = SLOT_RE.captures(trimmed).ok_or_else(err)?;
    let name = caps[1].to_string();
    let raw = caps[2].trim();
    let value = if let Some(inner) = raw.strip_prefix('"').and_then(|r| r.strip_suffix('"')) {
        SlotValue::Quoted(inner.to_string())
    } else if let Some(l) = parse_link_ref(raw) {
        SlotValue::Link(l)
    } else {
        if raw.is_empty() || raw.contains(char::is_whitespace) || STRAY_BRACKET_RE.is_match(raw) {
            return Err(err());
        }
        SlotValue::Bare(raw.to_string())
    };
    Ok(ParsedSlot { name, value, line: 0, span: None })
}

pub fn render_slot_line(s: &ParsedSlot) -> String {
    let v = match &s.value {
        SlotValue::Quoted(v) => format!("\"{v}\""),
        SlotValue::Bare(v) => v.clone(),
        SlotValue::Link(l) => format!("[{}](./{}.md)", l.title, l.slug),
    };
    format!("- {}: {v}", s.name)
}
```

  (Task 5 later factors the value classification/render into shared `classify_slot_value` / `render_slot_value`.)

- [ ] **Step 3.5: Green (grammar).** `cargo test -p waml --lib slot_`.

#### Phase B — wire into parse/serialize/build (TDD)

- [ ] **Step 3.6: Write the failing build + serialize tests.** In `serde_shape.rs`:

```rust
#[test]
fn instance_doc_slots_shape_and_ref_resolution() {
    let b = vec![
        ("m/ann.md".into(), "---\ntype: uml.Class\ntitle: Ann\n---\n# Ann\n".into()),
        ("m/order42.md".into(),
         "---\ntype: uml.InstanceSpecification\ntitle: order42\n---\n# order42\n\n## Slots\n- id: \"ORD-42\"\n- status: PLACED\n- owner: [Ann](./ann.md)\n".into()),
    ];
    let m = build_model(&b);
    let v = serde_json::to_value(&m).unwrap();
    let inst = v["nodes"].as_array().unwrap().iter().find(|n| n["key"] == "m/order42").unwrap();
    assert_eq!(inst["type"], "uml.InstanceSpecification");
    assert_eq!(inst["slots"][0]["name"], "id");
    assert_eq!(inst["slots"][0]["value"], "ORD-42");
    assert!(inst["slots"][0].get("ref").is_none());
    assert_eq!(inst["slots"][2]["value"], "Ann");
    assert_eq!(inst["slots"][2]["ref"], "m/ann");
}
```

  In `serialize.rs`:

```rust
    #[test]
    fn serialize_round_trips_slots_section() {
        let text = "---\ntype: uml.InstanceSpecification\ntitle: order42\n---\n# order42\n\n## Slots\n- id: \"ORD-42\"\n- status: PLACED\n- owner: [Ann](./ann.md)\n";
        let (doc, _) = crate::parse::parse(text);
        assert_eq!(crate::serialize::serialize_document(&doc), text, "## Slots must round-trip byte-identically");
    }
```

- [ ] **Step 3.7: Run, verify fail.** `cargo test -p waml --features serde --test serde_shape instance_doc_slots` + `cargo test -p waml --lib serialize_round_trips_slots`.

- [ ] **Step 3.8: Dispatch `"slots"` in `walk_section`** (parse.rs), next to `"attributes"`:

```rust
        "slots" => Section::Slots(walk_bullets(
            content, content_abs_start, src, DiagCode::DroppableContent,
            |line, ln| crate::grammar::parse_slot_line(line).map(|mut s| { s.line = ln; s }),
        )),
```

  Add `Section::Slots(v) => push_line_errors(v, &mut out),` to `diagnostics_of`.

- [ ] **Step 3.9: Populate `slots` in `build_node`** (parse.rs), adding a `resolve_slot` mirroring `resolve_attr`:

```rust
fn resolve_slot(s: &crate::syntax::ParsedSlot, referring_path: &str, keyset: &HashSet<&str>) -> crate::model::Slot {
    use crate::syntax::SlotValue;
    match &s.value {
        SlotValue::Quoted(v) | SlotValue::Bare(v) => crate::model::Slot { name: s.name.clone(), value: v.clone(), ref_: None },
        SlotValue::Link(l) => {
            let resolved = crate::okf::resolve_href(referring_path, &l.slug);
            crate::model::Slot { name: s.name.clone(), value: l.title.clone(), ref_: keyset.contains(resolved.as_str()).then_some(resolved) }
        }
    }
}
```

  In `build_node` add `let mut slots = Vec::new();`, a `Section::Slots(s) => slots = s.iter().filter_map(Line::parsed).map(|x| resolve_slot(x, &p.path, keyset)).collect(),` arm, and set `slots,` in the returned `Node`.

- [ ] **Step 3.10: Add `Section::Slots` to serialize** (serialize.rs): in `section_order` insert `Section::Slots(_) => 2,` (Attributes=1, Slots=2, Values=3, …; renumber the tail contiguously). In `render_section` add:

```rust
        Section::Slots(slots) => {
            let body = slots.iter().map(|l| match l {
                Line::Parsed(s) => crate::grammar::render_slot_line(s),
                Line::Error(e) => e.raw.clone(),
            }).collect::<Vec<_>>().join("\n");
            format!("## Slots\n{body}")
        }
```

- [ ] **Step 3.11: Green (Rust).** `cargo test --workspace && cargo clippy -p waml --all-targets && cargo fmt`.

- [ ] **Step 3.12: Confirm binding unchanged + TS green.** `pnpm build:wasm && git diff --exit-code packages/wasm/src/generated/waml_wasm.d.ts && pnpm lint && pnpm build && pnpm -r test`.

- [ ] **Step 3.13: Commit.**
  ```
  git add crates/waml/src/syntax.rs crates/waml/src/grammar.rs crates/waml/src/parse.rs crates/waml/src/serialize.rs crates/waml/tests/serde_shape.rs
  git commit -F - <<'EOF'
  feat(waml): parse and serialize the instance ## Slots section

  Plan: instances-object-diagrams
  Plan-Tasks: Task 3
  EOF
  ```

---

### Task 4: instance-of and links relationship verbs (parse/serialize + edges)

Teach the `## Relationships` grammar `instance of` and `links`; confirm they flow through the existing `build_edges` into `Model.edges` as `InstanceOf` / `Links`. **Rust-only, no binding change.**

**Guardrails:** `instance of` → `InstanceOf` (source instance, target classifier; no name, no ends). `links` → `Links` (source/target instances; optional `as [Assoc]` via existing `Edge.name`; no ends). Both are ordinary `Edge`s — do NOT special-case in `build_edges`; they fall through the generic non-`Associates` path. Validation is Task 6 (do NOT touch `validate.rs` here). No sequence/lifeline handling.

**Files:** `crates/waml/src/grammar.rs` (`REL_RE`, `rel_error_message`), tests in `grammar.rs`/`parse.rs`/`serde_shape.rs`.

- [ ] **Step 4.1: Write the failing grammar + build tests.** In `grammar.rs`:

```rust
    #[test]
    fn instance_of_and_links_relationships_round_trip() {
        for line in ["- instance of [Order](./order.md)", "- links [order42-line](./order42-line.md) as [Order→OrderLine](./order-orderline-assoc.md)"] {
            let r = parse_relationship_line(line).unwrap();
            assert_eq!(render_relationship_line(&r), line);
        }
        assert_eq!(parse_relationship_line("- instance of [Order](./order.md)").unwrap().kind, RelationshipKind::InstanceOf);
        let links = parse_relationship_line("- links [l](./l.md) as [A](./a.md)").unwrap();
        assert_eq!(links.kind, RelationshipKind::Links);
        assert!(matches!(links.name, Some(crate::syntax::ParsedName::Ref { .. })));
    }
```

  In `parse.rs`:

```rust
    #[test]
    fn instance_of_and_links_become_pool_edges() {
        let b = vec![
            ("m/order.md".into(), "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n".into()),
            ("m/assoc.md".into(), "---\ntype: uml.Association\ntitle: Order-Line\n---\n# Order-Line\n".into()),
            ("m/line42.md".into(), "---\ntype: uml.InstanceSpecification\ntitle: line42\n---\n# line42\n".into()),
            ("m/order42.md".into(),
             "---\ntype: uml.InstanceSpecification\ntitle: order42\n---\n# order42\n\n## Relationships\n- instance of [Order](./order.md)\n- links [line42](./line42.md) as [Order-Line](./assoc.md)\n".into()),
        ];
        let m = build_model(&b);
        let io = m.edges.iter().find(|e| e.kind == RelationshipKind::InstanceOf).unwrap();
        assert_eq!((io.source.as_str(), io.target.as_str()), ("m/order42", "m/order"));
        let lk = m.edges.iter().find(|e| e.kind == RelationshipKind::Links).unwrap();
        assert_eq!((lk.source.as_str(), lk.target.as_str()), ("m/order42", "m/line42"));
        assert_eq!(lk.name, Some(crate::model::AssocName::Assoc("m/assoc".into())));
    }
```

- [ ] **Step 4.2: Run, verify fail.** `cargo test -p waml --lib instance_of_and_links`.

- [ ] **Step 4.3: Extend `REL_RE`** — append `|instance of|links` to the verb alternation:

```rust
        r"^- (associates|aggregates|composes|specializes|implements|depends|includes|extends|instance of|links) ",
```

- [ ] **Step 4.4: Extend `rel_error_message`** if it enumerates verbs (else leave). Do NOT change its structure.

- [ ] **Step 4.5: Green (Rust).** `cargo test --workspace && cargo clippy -p waml --all-targets && cargo fmt` — `build_edges` needed no change.

- [ ] **Step 4.6: Confirm binding unchanged + TS green.** `pnpm build:wasm && git diff --exit-code packages/wasm/src/generated/waml_wasm.d.ts && pnpm lint && pnpm build && pnpm -r test`.

- [ ] **Step 4.7: Commit.**
  ```
  git add crates/waml/src/grammar.rs crates/waml/src/parse.rs
  git commit -F - <<'EOF'
  feat(waml): parse instance-of and links relationship verbs into edges

  Plan: instances-object-diagrams
  Plan-Tasks: Task 4
  EOF
  ```

---

### Task 5: Inline instances in a diagram (## Members promotion)

Author an object inline in `## Members`; promote each to a pool `InstanceSpecification` `Node` keyed `{diagram}#name` (mirroring `build_flows`), auto-added to the diagram's `members`, with an `InstanceOf` edge to its classifier. **Rust-only, no binding change.**

**Guardrails:** instances-only (do NOT generalize to arbitrary elements). Promotion mirrors `build_flows`. Slots reuse Task-3 `SlotValue`/`ParsedSlot`; canonical serialize joins with ` and ` for byte-identical round-trip. Reserved tokens `instance of`/`with`/`set to`/`and`/`as`. A plain `- [Title](./slug.md)` member is unchanged. Validation is Task 6; here only adapt `check_group_members` to the new item type (compile-forced, mechanical — no new diagnostics).

**Files:** `syntax.rs`, `grammar.rs`, `parse.rs`, `validate.rs`, `serialize.rs` (via members render), tests.

**Interfaces — Produces:** `enum MemberItem { Member(MemberLine), Instance(InlineInstance) }`; `struct InlineInstance { classifier: LinkRef, name, slots: Vec<ParsedSlot>, line, span }`; `MemberGroup.members: Vec<Line<MemberItem>>`; `parse_inline_instance` / `render_inline_instance`; `build_diagrams(...) -> (Vec<Diagram>, Vec<Node>, Vec<Edge>)`.

#### Phase A — syntax + grammar (TDD)

- [ ] **Step 5.1: Write the failing round-trip test** in `grammar.rs`:

```rust
    #[test]
    fn inline_instance_lines_round_trip() {
        for line in [
            "- instance of [Order](./order.md) as order42",
            "- instance of [Order](./order.md) as order42 with id set to \"ORD-42\" and status set to PLACED",
            "- instance of [Order](./order.md) as o with owner set to [Ann](./ann.md)",
        ] {
            let i = parse_inline_instance(line).unwrap();
            assert_eq!(render_inline_instance(&i), line);
        }
        let i = parse_inline_instance("- instance of [Order](./order.md) as order42 with id set to \"ORD-42\" and status set to PLACED").unwrap();
        assert_eq!((i.classifier.title.as_str(), i.classifier.slug.as_str(), i.name.as_str()), ("Order", "order", "order42"));
        assert_eq!(i.slots.len(), 2);
    }
```

- [ ] **Step 5.2: Run, verify fail to compile.** `cargo test -p waml --lib inline_instance`.

- [ ] **Step 5.3: Add the syntax types + change `MemberGroup.members`** in `syntax.rs`:

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum MemberItem {
    Member(MemberLine),
    Instance(InlineInstance),
}

#[derive(Debug, Clone, PartialEq)]
pub struct InlineInstance {
    pub classifier: LinkRef,
    pub name: String,
    pub slots: Vec<ParsedSlot>,
    pub line: usize,
    pub span: Option<(usize, usize)>,
}
```

  Change `MemberGroup.members` to `Vec<Line<MemberItem>>`.

- [ ] **Step 5.4: Add the grammar** in `grammar.rs` (import `InlineInstance`, `MemberItem`), factoring the shared value helpers and refactoring Task 3's `parse_slot_line`/`render_slot_line` to call them:

```rust
static INLINE_INSTANCE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^- instance of \[([^\]]+)\]\(\./(.+?)\.md\) as ([A-Za-z_][A-Za-z0-9_]*)(?: with (.+))?$").unwrap()
});
static SLOT_ASSIGN_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"^([A-Za-z_][A-Za-z0-9_]*) set to ("[^"]*"|\[[^\]]+\]\(\./.+?\.md\)|\S+)(?: and (.*))?$"#).unwrap()
});

pub fn classify_slot_value(raw: &str) -> Option<SlotValue> {
    let raw = raw.trim();
    if let Some(inner) = raw.strip_prefix('"').and_then(|r| r.strip_suffix('"')) {
        Some(SlotValue::Quoted(inner.to_string()))
    } else if let Some(l) = parse_link_ref(raw) {
        Some(SlotValue::Link(l))
    } else if raw.is_empty() || raw.contains(char::is_whitespace) || STRAY_BRACKET_RE.is_match(raw) {
        None
    } else {
        Some(SlotValue::Bare(raw.to_string()))
    }
}

pub fn render_slot_value(v: &SlotValue) -> String {
    match v {
        SlotValue::Quoted(s) => format!("\"{s}\""),
        SlotValue::Bare(s) => s.clone(),
        SlotValue::Link(l) => format!("[{}](./{}.md)", l.title, l.slug),
    }
}

fn parse_slot_clause(clause: &str, whole: &str) -> Result<Vec<ParsedSlot>, LineError> {
    let err = || LineError { range: bullet_range(whole), message: "malformed instance slot clause — expected '<name> set to <value>[ and …]'".to_string() };
    let mut out = Vec::new();
    let mut rest = clause.trim().to_string();
    while !rest.is_empty() {
        let caps = SLOT_ASSIGN_RE.captures(&rest).ok_or_else(err)?;
        let name = caps[1].to_string();
        let value = classify_slot_value(&caps[2]).ok_or_else(err)?;
        out.push(ParsedSlot { name, value, line: 0, span: None });
        rest = caps.get(3).map(|m| m.as_str().trim().to_string()).unwrap_or_default();
    }
    Ok(out)
}

pub fn parse_inline_instance(line: &str) -> Result<InlineInstance, LineError> {
    let err = || LineError { range: bullet_range(line), message: "malformed inline instance — expected '- instance of [Title](./slug.md) as <name>[ with <n> set to <v> and …]'".to_string() };
    let trimmed = line.trim_end_matches('\r').trim();
    let caps = INLINE_INSTANCE_RE.captures(trimmed).ok_or_else(err)?;
    let classifier = LinkRef { title: caps[1].to_string(), slug: caps[2].to_string() };
    let name = caps[3].to_string();
    let slots = match caps.get(4) { Some(c) => parse_slot_clause(c.as_str(), trimmed)?, None => Vec::new() };
    Ok(InlineInstance { classifier, name, slots, line: 0, span: None })
}

pub fn render_inline_instance(i: &InlineInstance) -> String {
    let mut s = format!("- instance of [{}](./{}.md) as {}", i.classifier.title, i.classifier.slug, i.name);
    if !i.slots.is_empty() {
        let clause = i.slots.iter().map(|sl| format!("{} set to {}", sl.name, render_slot_value(&sl.value))).collect::<Vec<_>>().join(" and ");
        s.push_str(&format!(" with {clause}"));
    }
    s
}
```

  Then in `parse_slot_line` use `classify_slot_value(caps[2].trim()).ok_or_else(err)?` and in `render_slot_line` use `render_slot_value(&s.value)`.

- [ ] **Step 5.5: Teach `parse_members_block`** — replace the `parse_member_line` match with a member→instance→error cascade producing `Line::Parsed(MemberItem::Member(m))` / `Line::Parsed(MemberItem::Instance(inst))` (positions set via `crate::parse::find_link_span` on the classifier link) / the existing `DroppableContent` error.

- [ ] **Step 5.6: Teach `render_members_block`** — in `render_group`, match `MemberItem::Member(ml) => render_member_line(ml)`, `MemberItem::Instance(i) => render_inline_instance(i)`, `Line::Error(e) => e.raw`.

- [ ] **Step 5.7: Green (grammar).** `cargo test -p waml --lib inline_instance && cargo test -p waml --lib slot_`.

#### Phase B — promotion in parse (TDD)

- [ ] **Step 5.8: Write the failing promotion + serialize tests.** In `serde_shape.rs`:

```rust
#[test]
fn inline_instance_is_promoted_to_a_pool_node_with_edge_and_membership() {
    let b = vec![
        ("m/order.md".into(), "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n".into()),
        ("m/objects.md".into(),
         "---\ntype: Diagram\ntitle: Objects\nprofile: uml-domain\n---\n# Objects\n\n## Members\n- [Order](./order.md)\n- instance of [Order](./order.md) as order42 with id set to \"ORD-42\" and status set to PLACED\n".into()),
    ];
    let m = build_model(&b);
    let v = serde_json::to_value(&m).unwrap();
    let inst = v["nodes"].as_array().unwrap().iter().find(|n| n["key"] == "m/objects#order42").unwrap();
    assert_eq!(inst["type"], "uml.InstanceSpecification");
    assert_eq!(inst["slots"][0]["value"], "ORD-42");
    let io = v["edges"].as_array().unwrap().iter().find(|e| e["kind"] == "instanceof" && e["from"] == "m/objects#order42").unwrap();
    assert_eq!(io["to"], "m/order");
    let members = &v["diagrams"][0]["groups"][0]["members"];
    assert!(members.as_array().unwrap().iter().any(|k| k == "m/objects#order42"));
}
```

  In `serialize.rs`:

```rust
    #[test]
    fn serialize_round_trips_inline_instance_member() {
        let text = "---\ntype: Diagram\ntitle: Objects\nprofile: uml-domain\n---\n# Objects\n\n## Members\n- [Order](./order.md)\n- instance of [Order](./order.md) as order42 with id set to \"ORD-42\" and status set to PLACED\n";
        let (doc, _) = crate::parse::parse(text);
        assert_eq!(crate::serialize::serialize_document(&doc), text);
    }
```

- [ ] **Step 5.9: Run, verify fail.** `cargo test -p waml --features serde --test serde_shape inline_instance_is_promoted`.

- [ ] **Step 5.10: Thread the diagram key + promotion through `build_diagrams`** (parse.rs). Change `resolve_group` to take `diagram_key: &str` and resolve `MemberItem::Member` via keyset and `MemberItem::Instance` to `format!("{diagram_key}#{}", inst.name)`. Add a recursive `promote_inline_instances(g, p, diagram_key, keyset, &mut nodes, &mut edges)` that builds each promoted `Node { ty: InstanceSpecification, key: "{diagram}#{name}", slots: inst.slots.map(resolve_slot), concept: okf::project("{key}.md", "# {name}\n") with title set, .. }` and, when the classifier resolves in keyset, an `Edge { source: node_key, target, kind: InstanceOf, name: None, from_end/to_end default, bidirectional: false }`. Change `build_diagrams` to return `(Vec<Diagram>, Vec<Node>, Vec<Edge>)`, calling both per `Section::Members` block. Ensure `Edge`/`RelEnd`/`UmlMetaclass`/`RelationshipKind` are imported (or fully-qualified). (Full code in `task-5-inline-instances.md`.)

- [ ] **Step 5.11: Merge in `build_model`** — `let mut nodes = …; let mut edges = build_edges(…); let (diagrams, inst_nodes, inst_edges) = build_diagrams(&parsed, &keyset); nodes.extend(inst_nodes); edges.extend(inst_edges);`. Leave packages/flows/interactions and the returned `Model` unchanged.

- [ ] **Step 5.12: Adapt `validate.rs::check_group_members`** to `let MemberItem::Member(m) = item else { continue };` (skip `Instance` — validated in Task 6). Fix any other compiler-flagged `g.members` site (grep `g.members` / `.members.iter()`). `push_group_errors` needs no change.

- [ ] **Step 5.13: Green (Rust).** `cargo test --workspace && cargo clippy -p waml --all-targets && cargo fmt`.

- [ ] **Step 5.14: Confirm binding unchanged + TS green.** `pnpm build:wasm && git diff --exit-code packages/wasm/src/generated/waml_wasm.d.ts && pnpm lint && pnpm build && pnpm -r test`.

- [ ] **Step 5.15: Commit.**
  ```
  git add crates/waml/src/syntax.rs crates/waml/src/grammar.rs crates/waml/src/parse.rs crates/waml/src/validate.rs crates/waml/src/serialize.rs crates/waml/tests/serde_shape.rs
  git commit -F - <<'EOF'
  feat(waml): promote inline diagram instances to pool nodes with edges

  Plan: instances-object-diagrams
  Plan-Tasks: Task 5
  EOF
  ```

---

### Task 6: Instance conformance validation (three warn-only diagnostics)

Add the three warn-only conformance diagnostics. This adds `DiagCode` variants (a tsify enum → the binding regenerates), so Rust + regen land in ONE commit — the slice's second/final regen.

**Guardrails (spec §5):** `SlotUnknownAttribute` (slot name not an attribute of the classifier), `InstanceOfNonClassifier` (`instance of` target `is_classifier() == false`, incl. another instance), `InstanceOfUnresolved` (dangling classifier ref) — all `Diagnostic::warn`. Validate BOTH standalone docs and inline diagram instances. The generic `## Relationships` `UnresolvedTarget` (Error) loop SKIPS `InstanceOf`/`Links`. Do NOT hand-edit generated files.

**Files:** `crates/waml/src/diagnostic.rs`, `crates/waml/src/validate.rs`, regenerated binding, tests in `validate.rs`.

#### Phase A — DiagCodes (TDD)

- [ ] **Step 6.1: Write the failing validation tests** in `validate.rs` (call the public `validate(&bundle)`): `instance_of_unresolved_classifier_warns`, `instance_of_non_classifier_target_warns`, `slot_unknown_attribute_warns`, `conformant_instance_produces_no_instance_warnings`. (Full code in `task-6-instance-conformance-validation.md`.) Assert each fires with `Severity::Warning`, that an unresolved instance-of does NOT surface as a hard `UnresolvedTarget`, that only the unknown slot warns, and that a conformant instance emits none of the three.

- [ ] **Step 6.2: Run, verify fail to compile.** `cargo test -p waml --lib instance_of_unresolved`.

- [ ] **Step 6.3: Add the `DiagCode` variants** in `diagnostic.rs` — `SlotUnknownAttribute`, `InstanceOfNonClassifier`, `InstanceOfUnresolved`; `as_str` arms `slot-unknown-attribute` / `instance-of-non-classifier` / `instance-of-unresolved`; add all three to the `severity()` Warning arm.

- [ ] **Step 6.4: Add `check_instance_of_target`** (free fn) in `validate.rs` — `!in_keyset` → `InstanceOfUnresolved` warn; resolved-but-not-classifier → `InstanceOfNonClassifier` warn; both via `Diagnostic::warn` with the span when present.

- [ ] **Step 6.5: Build the attribute-name lookup + skip instance kinds.** In `link`, after `types`, add `docs_by_key` and an `attr_names_of` closure collecting `Section::Attributes` names. In the generic `Section::Relationships` arm, `if matches!(r.kind, RelationshipKind::InstanceOf | RelationshipKind::Links) { continue; }` at the top of the per-rel body.

- [ ] **Step 6.6: Add the instance-conformance pass.** After the `for s in &doc.sections { match s { … } }` block: for `ty == InstanceSpecification`, walk `Section::Relationships` `InstanceOf` targets (call `check_instance_of_target`; remember the first resolved classifier) then check each `Section::Slots` name against `attr_names_of(classifier)`; for `ty == Diagram`, recursively walk `Section::Members` `MemberItem::Instance` items the same way. (Full code in `task-6-instance-conformance-validation.md`.)

- [ ] **Step 6.7: Green (Rust).** `cargo test --workspace && cargo clippy -p waml --all-targets && cargo fmt`.

#### Phase C — regen binding + gate + commit

- [ ] **Step 6.8: Regenerate.** `pnpm build:wasm` then `git diff packages/wasm/src/generated/waml_wasm.d.ts` — the `DiagCode` union gains the three kebab strings. Do NOT hand-edit.

- [ ] **Step 6.9: Green (TS).** `pnpm lint && pnpm build && pnpm -r test`.

- [ ] **Step 6.10: Commit.**
  ```
  git add crates/waml/src/diagnostic.rs crates/waml/src/validate.rs packages/wasm/src/generated/waml_wasm.d.ts packages/wasm/src/generated/waml_wasm.js
  git commit -F - <<'EOF'
  feat(waml): warn-only instance conformance validation

  Plan: instances-object-diagrams
  Plan-Tasks: Task 6
  EOF
  ```
  (Add any other regenerated `packages/wasm/src/generated/*` per `git status`.)

---

## Self-Review

**1. Spec coverage.** §3.1 → T1; §3.2/§3.3/§3.4 → T2; §4.1 → T3; §4.3/§4.4 → T4; §4.2 → T5; §5 → T6; §6 (bindings + TS `slots`, additive edge kinds) → T2 & T6; §7 (atomic coupled shape) → T2 & T6 each one commit; §8 (serde-shape, grammar round-trip, per-code validation) → tests throughout. Guards (§2/§9) restated per task.

**2. Placeholder scan.** No TBD / "handle edge cases" / "similar to". Every code step shows complete code or names the exact existing pattern it mirrors (`resolve_attr`, `build_flows`) with concrete resulting code.

**3. Type consistency.** `Slot`, `SlotValue::{Quoted,Bare,Link}`, `ParsedSlot`, `MemberItem::{Member,Instance}`, `InlineInstance`, `RelationshipKind::{InstanceOf,Links}`, `DiagCode::{SlotUnknownAttribute,InstanceOfNonClassifier,InstanceOfUnresolved}` are named identically across model/syntax/grammar/parse/validate/tests/binding/TS. `classify_slot_value`/`render_slot_value` are the single shared surface-form helpers.

## Cross-slice contract

Instances are pooled (§5) — `InstanceSpecification` is a `Model.nodes` member with `is_classifier() == false`. `InstanceOf`/`Links` are structural `Model.edges`, distinct from behavior `flow_edges` and interaction-local `SeqEdge`s — no shared-pool collision. `Lifeline.ref_` stays classifier-only (slice §7.4 deferred); this slice adds no sequence handling.
