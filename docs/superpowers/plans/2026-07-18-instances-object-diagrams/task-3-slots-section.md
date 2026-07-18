# Instances + Object Diagrams — Task 3: Standalone instance `## Slots` section

> **Segment 3 of 6** of the **Instances + Object Diagrams** plan (slug `instances-object-diagrams`). See [`README.md`](README.md) for Goal, Global Constraints, File Structure, and the full plan preserved verbatim as [`_source.md`](_source.md).
> **REQUIRED SUB-SKILL:** superpowers:subagent-driven-development (recommended) or superpowers:executing-plans. Steps use checkbox (`- [ ]`) syntax for tracking.

### Task 3: Standalone instance Slots section (## Slots parse/serialize + build)

Wire the new `## Slots` section end-to-end: syntax type + grammar parse/render + section dispatch + canonical serialize + `build_node` population. A standalone `type: uml.InstanceSpecification` doc with a `## Slots` list now produces a pool `Node` with populated `slots`. **Rust-only, no binding change** (the `Slot` runtime type + `Node.slots` already landed in Task 2 and are unchanged), so `pnpm build:wasm` regenerates an identical binding — independently green under the full gate.

**Guardrails (from the design spec):**
- `## Slots` lines are `- name: value`. A slot **value** is a quoted string (`"ORD-42"`), a bare identifier/number (`PLACED`, `3`), or a `[Label](./ref.md)` link (instance-valued slot → resolved `Slot.ref_`).
- **Round-trip must be byte-identical.** Quoting is part of the surface form: `- id: "ORD-42"` must serialize back to `- id: "ORD-42"`, and `- status: PLACED` back to `- status: PLACED`. Preserve the surface form in the **syntax** layer (`SlotValue`), distinct from the resolved runtime `Slot`.
- Do NOT redesign or reorder the existing section grammar; `## Slots` is a purely additive production.

**Files:**
- Modify: `crates/waml/src/syntax.rs` — add `enum SlotValue`, `struct ParsedSlot`; add `Section::Slots(Vec<Line<ParsedSlot>>)`.
- Modify: `crates/waml/src/grammar.rs` — add `parse_slot_line` / `render_slot_line` (+ a `SLOT_RE`); export the `ParsedSlot`/`SlotValue` imports.
- Modify: `crates/waml/src/parse.rs` — dispatch `"slots"` in `walk_section`; add `Section::Slots` to `diagnostics_of`; build a resolved `slots` in `build_node`.
- Modify: `crates/waml/src/serialize.rs` — add `Section::Slots` to `section_order` and `render_section`.
- Test: `crates/waml/src/grammar.rs` (round-trip unit tests), `crates/waml/tests/serde_shape.rs` (standalone-instance shape), `crates/waml/src/parse.rs` (build test).

**Interfaces:**
- Produces (Rust, `crate::syntax`):
  - `pub enum SlotValue { Quoted(String), Bare(String), Link(LinkRef) }`.
  - `pub struct ParsedSlot { pub name: String, pub value: SlotValue, pub line: usize, pub span: Option<(usize, usize)> }`.
  - `Section::Slots(Vec<Line<ParsedSlot>>)`.
- Produces (Rust, `crate::grammar`): `pub fn parse_slot_line(line: &str) -> Result<ParsedSlot, LineError>`; `pub fn render_slot_line(s: &ParsedSlot) -> String` (exact inverse).
- Consumes: `crate::model::Slot` (Task 2), `LinkRef`, `resolve_href` / keyset resolution (mirror `resolve_attr`).

### Phase A — syntax + grammar (TDD)

- [ ] **Step 3.1: Write the failing grammar round-trip test.** In `crates/waml/src/grammar.rs` (inline `#[cfg(test)] mod tests`), add:

```rust
    #[test]
    fn slot_lines_round_trip_all_three_value_forms() {
        for line in [
            "- id: \"ORD-42\"",
            "- status: PLACED",
            "- qty: 3",
            "- customer: [Ann](./ann.md)",
        ] {
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

- [ ] **Step 3.2: Run it, verify it fails to compile.** Run:
  ```
  cargo test -p waml --lib slot_
  ```
  Expected: FAIL to compile — `parse_slot_line` / `SlotValue` do not exist.

- [ ] **Step 3.3: Add the syntax types.** In `crates/waml/src/syntax.rs`, add near `ParsedRel`:

```rust
/// A `## Slots` value's SURFACE form (preserved for byte-identical round-trip),
/// distinct from the resolved `model::Slot`.
#[derive(Debug, Clone, PartialEq)]
pub enum SlotValue {
    /// A `"quoted string"` literal (quotes are part of the surface form).
    Quoted(String),
    /// A bare identifier or number (`PLACED`, `3`).
    Bare(String),
    /// A `[Label](./slug.md)` link (instance-valued slot); resolved downstream.
    Link(LinkRef),
}

/// One `## Slots` bullet: `- name: value`.
#[derive(Debug, Clone, PartialEq)]
pub struct ParsedSlot {
    pub name: String,
    pub value: SlotValue,
    /// 1-based line within the document (0 until filled by `parse`).
    pub line: usize,
    /// Byte range within `line`, if positioned by `parse`.
    pub span: Option<(usize, usize)>,
}
```

  And add the variant to `pub enum Section`:

```rust
    /// An `InstanceSpecification`'s slot values.
    Slots(Vec<Line<ParsedSlot>>),
```

  (`LinkRef` is already defined in `syntax.rs`.)

- [ ] **Step 3.4: Add `parse_slot_line` / `render_slot_line`.** In `crates/waml/src/grammar.rs`, add a regex near the others and the two functions. Import `ParsedSlot`, `SlotValue` in the `use crate::syntax::{…}` block.

```rust
static SLOT_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^- ([A-Za-z_][A-Za-z0-9_]*): (.+)$").unwrap());

/// Parse `- name: value` where value is a quoted string, a `[Label](./slug.md)`
/// link, or a bare identifier/number. The value's surface form is preserved in
/// `SlotValue` for byte-identical round-trip.
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
        // A bare value must be a single token with no whitespace / stray brackets.
        if raw.is_empty() || raw.contains(char::is_whitespace) || STRAY_BRACKET_RE.is_match(raw) {
            return Err(err());
        }
        SlotValue::Bare(raw.to_string())
    };
    Ok(ParsedSlot { name, value, line: 0, span: None })
}

/// Exact inverse of `parse_slot_line`.
pub fn render_slot_line(s: &ParsedSlot) -> String {
    let v = match &s.value {
        SlotValue::Quoted(v) => format!("\"{v}\""),
        SlotValue::Bare(v) => v.clone(),
        SlotValue::Link(l) => format!("[{}](./{}.md)", l.title, l.slug),
    };
    format!("- {}: {v}", s.name)
}
```

- [ ] **Step 3.5: Run the grammar tests, verify green.** Run:
  ```
  cargo test -p waml --lib slot_
  ```
  Expected: PASS — both round-trip and classification tests.

### Phase B — wire the section into parse/serialize/build (TDD)

- [ ] **Step 3.6: Write the failing build + serde-shape tests.** In `crates/waml/tests/serde_shape.rs` add:

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
    assert_eq!(inst["slots"][2]["name"], "owner");
    assert_eq!(inst["slots"][2]["value"], "Ann");
    assert_eq!(inst["slots"][2]["ref"], "m/ann", "link-valued slot resolves to a pool key");
}
```

  And add a serialize round-trip test in `crates/waml/src/serialize.rs` (inline tests) proving the section is a fixpoint:

```rust
    #[test]
    fn serialize_round_trips_slots_section() {
        let text = "---\ntype: uml.InstanceSpecification\ntitle: order42\n---\n# order42\n\n## Slots\n- id: \"ORD-42\"\n- status: PLACED\n- owner: [Ann](./ann.md)\n";
        let (doc, _) = crate::parse::parse(text);
        let once = crate::serialize::serialize_document(&doc);
        assert_eq!(once, text, "## Slots must round-trip byte-identically");
    }
```

- [ ] **Step 3.7: Run them, verify they fail.** Run:
  ```
  cargo test -p waml --features serde --test serde_shape instance_doc_slots
  cargo test -p waml --lib serialize_round_trips_slots
  ```
  Expected: FAIL — `## Slots` currently parses as `Section::Unknown` (slots empty; serialize drops the surface form) and `build_node` does not read it.

- [ ] **Step 3.8: Dispatch `"slots"` in `walk_section`.** In `crates/waml/src/parse.rs`, add the arm (next to `"attributes"`), reusing `walk_bullets`:

```rust
        "slots" => Section::Slots(walk_bullets(
            content,
            content_abs_start,
            src,
            DiagCode::DroppableContent,
            |line, ln| {
                crate::grammar::parse_slot_line(line).map(|mut s| {
                    s.line = ln;
                    s
                })
            },
        )),
```

  (`DroppableContent` is the tolerant code used for `## Values`/`## Notes`; a malformed slot line is preserved as a positioned error, never dropped.) Add `Section::Slots(v) => push_line_errors(v, &mut out),` to `diagnostics_of`.

- [ ] **Step 3.9: Populate `slots` in `build_node`.** In `crates/waml/src/parse.rs`, add a resolver mirroring `resolve_attr`, and read `Section::Slots` in `build_node`:

```rust
fn resolve_slot(s: &crate::syntax::ParsedSlot, referring_path: &str, keyset: &HashSet<&str>) -> crate::model::Slot {
    use crate::syntax::SlotValue;
    match &s.value {
        SlotValue::Quoted(v) | SlotValue::Bare(v) => crate::model::Slot {
            name: s.name.clone(),
            value: v.clone(),
            ref_: None,
        },
        SlotValue::Link(l) => {
            let resolved = crate::okf::resolve_href(referring_path, &l.slug);
            crate::model::Slot {
                name: s.name.clone(),
                value: l.title.clone(),
                ref_: keyset.contains(resolved.as_str()).then_some(resolved), // else degrade to display token
            }
        }
    }
}
```

  In `build_node`, add a `let mut slots = Vec::new();` alongside `attributes`/`values`, a match arm `Section::Slots(s) => slots = s.iter().filter_map(Line::parsed).map(|x| resolve_slot(x, &p.path, keyset)).collect(),`, and set `slots,` in the returned `Node { … }` (replacing the `slots: Vec::new()` placeholder from Task 2).

- [ ] **Step 3.10: Add `Section::Slots` to serialize.** In `crates/waml/src/serialize.rs`:
  - In `section_order`, insert `Section::Slots(_) => 2,` and renumber the tail so ordering stays contiguous and `## Slots` sits just after `## Attributes` (Attributes=1, Slots=2, Values=3, Relationships=4, …). Update every subsequent arm's number accordingly.
  - In `render_section`, add:
    ```rust
        Section::Slots(slots) => {
            let body = slots
                .iter()
                .map(|l| match l {
                    Line::Parsed(s) => crate::grammar::render_slot_line(s),
                    Line::Error(e) => e.raw.clone(),
                })
                .collect::<Vec<_>>()
                .join("\n");
            format!("## Slots\n{body}")
        }
    ```

- [ ] **Step 3.11: Run the full Rust gate, verify green.** Run:
  ```
  cargo test --workspace
  cargo clippy -p waml --all-targets
  cargo fmt
  ```
  Expected: all green, including the new build/serde/serialize tests and every existing round-trip test (section reordering in `section_order` is internal — canonical output order is unchanged for docs without `## Slots`).

- [ ] **Step 3.12: Run the TS half of the gate (no binding change expected).** Run:
  ```
  pnpm build:wasm
  git diff --exit-code packages/wasm/src/generated/waml_wasm.d.ts
  pnpm lint && pnpm build && pnpm -r test
  ```
  Expected: `git diff --exit-code` reports NO change to the binding (this task added no tsify-visible type); the TS gate stays green.

- [ ] **Step 3.13: Commit.** Run:
  ```
  git add crates/waml/src/syntax.rs crates/waml/src/grammar.rs crates/waml/src/parse.rs crates/waml/src/serialize.rs crates/waml/tests/serde_shape.rs
  git commit -F - <<'EOF'
  feat(waml): parse and serialize the instance ## Slots section

  Plan: instances-object-diagrams
  Plan-Tasks: Task 3
  EOF
  ```
