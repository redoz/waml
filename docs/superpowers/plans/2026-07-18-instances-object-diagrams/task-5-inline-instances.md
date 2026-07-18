# Instances + Object Diagrams — Task 5: Inline instances in a diagram (`## Members` promotion)

> **Segment 5 of 6** of the **Instances + Object Diagrams** plan (slug `instances-object-diagrams`). See [`README.md`](README.md) for Goal, Global Constraints, File Structure, and the full plan preserved verbatim as [`_source.md`](_source.md).
> **REQUIRED SUB-SKILL:** superpowers:subagent-driven-development (recommended) or superpowers:executing-plans. Steps use checkbox (`- [ ]`) syntax for tracking.

### Task 5: Inline instances in a diagram (## Members promotion)

Let a Diagram author an object inline in `## Members`:
`- instance of [Order](./order.md) as order42 with id set to "ORD-42" and status set to PLACED`.
Each such line is **promoted** to a pool `InstanceSpecification` `Node` keyed `{diagram}#order42` (mirroring `build_flows`' `{behavior}#id` pooling), auto-added to that diagram's `members`, and given an `InstanceOf` edge to its classifier. **Rust-only, no binding change** — independently green.

**Guardrails (from the design spec):**
- Inline is **instances-only**. Do NOT generalize `## Members` to inline arbitrary elements (whole classes, etc.) — that is a separate future idea, explicitly out of scope.
- Promotion mirrors `build_flows`: pool key `{diagram}#{name}`, the value auto-added to the diagram's member list; the promoted node is a real `Model.nodes` member and the `instance of` link becomes a real `Model.edges` `InstanceOf` edge.
- Slots come from the `with … set to … and …` clause; reuse the Task-3 `SlotValue`/`ParsedSlot` surface types so the line round-trips byte-identically. **Canonical serialize** joins slots with ` and ` (`<name> set to <value>`), one form, so parse → serialize is byte-identical for canonically-authored lines.
- Reserved tokens inside the clause: `instance of`, `with`, `set to`, `and`, `as`. A bare value may not contain a reserved word (quote it); a value is a quoted string, a bare ident/number, or a `[Label](./ref.md)` link.
- Do NOT redesign the existing member-line grammar; the inline-instance line is an additive alternative bullet. A plain `- [Title](./slug.md)` member is unchanged.
- Validation of inline instances is **Task 6**; here, only adapt `validate.rs::check_group_members` to the new member-item type (a compile-forced, mechanical change — do NOT add diagnostics).

**Files:**
- Modify: `crates/waml/src/syntax.rs` — add `enum MemberItem`, `struct InlineInstance`; change `MemberGroup.members` to `Vec<Line<MemberItem>>`.
- Modify: `crates/waml/src/grammar.rs` — add `parse_inline_instance`, `render_inline_instance`, a shared `classify_slot_value`/`render_slot_value` (factored from Task 3's `parse_slot_line`/`render_slot_line`), a `parse_slot_clause`; teach `parse_members_block` + `render_members_block` the new item.
- Modify: `crates/waml/src/parse.rs` — thread the diagram key + inline-instance promotion through `resolve_group` and `build_diagrams` (now returns `(Vec<Diagram>, Vec<Node>, Vec<Edge>)`); extend `build_model` to merge the promoted nodes/edges.
- Modify: `crates/waml/src/validate.rs` — adapt `check_group_members` to `MemberItem::Member`.
- Test: `crates/waml/src/grammar.rs` (round-trip), `crates/waml/src/parse.rs` (promotion), `crates/waml/tests/serde_shape.rs` (promoted-node + edge shape).

**Interfaces:**
- Produces (Rust, `crate::syntax`):
  - `pub enum MemberItem { Member(MemberLine), Instance(InlineInstance) }`.
  - `pub struct InlineInstance { pub classifier: LinkRef, pub name: String, pub slots: Vec<ParsedSlot>, pub line: usize, pub span: Option<(usize, usize)> }`.
  - `MemberGroup.members: Vec<Line<MemberItem>>`.
- Produces (Rust, `crate::grammar`): `pub fn parse_inline_instance(line: &str) -> Result<InlineInstance, LineError>`; `pub fn render_inline_instance(i: &InlineInstance) -> String` (exact inverse).
- Produces (Rust, `crate::parse`): `build_diagrams(...) -> (Vec<Diagram>, Vec<Node>, Vec<Edge>)`; a promoted `Node { ty: InstanceSpecification, key: "{diagram}#{name}", slots, .. }` and an `Edge { kind: InstanceOf, source: "{diagram}#{name}", target: classifier_key }`.
- Consumes: `SlotValue`/`ParsedSlot` (Task 3), `Slot`/`RelationshipKind::InstanceOf`/`InstanceSpecification` (Tasks 1–2), `okf::project`/`id_of`/`resolve_href`.

### Phase A — syntax + grammar (TDD)

- [ ] **Step 5.1: Write the failing grammar round-trip test.** In `crates/waml/src/grammar.rs` tests add:

```rust
    #[test]
    fn inline_instance_lines_round_trip() {
        for line in [
            "- instance of [Order](./order.md) as order42",
            "- instance of [Order](./order.md) as order42 with id set to \"ORD-42\" and status set to PLACED",
            "- instance of [Order](./order.md) as o with owner set to [Ann](./ann.md)",
        ] {
            let i = parse_inline_instance(line).unwrap();
            assert_eq!(render_inline_instance(&i), line, "inline instance must round-trip byte-identically");
        }
        let i = parse_inline_instance("- instance of [Order](./order.md) as order42 with id set to \"ORD-42\" and status set to PLACED").unwrap();
        assert_eq!((i.classifier.title.as_str(), i.classifier.slug.as_str(), i.name.as_str()), ("Order", "order", "order42"));
        assert_eq!(i.slots.len(), 2);
        assert_eq!(i.slots[0].name, "id");
        assert_eq!(i.slots[1].name, "status");
    }
```

- [ ] **Step 5.2: Run it, verify it fails to compile.** Run:
  ```
  cargo test -p waml --lib inline_instance
  ```
  Expected: FAIL to compile — `parse_inline_instance` does not exist.

- [ ] **Step 5.3: Add the syntax types + change `MemberGroup.members`.** In `crates/waml/src/syntax.rs`:

```rust
/// One `## Members` bullet: a plain member ref, or an inline instance that is
/// promoted to a pool `InstanceSpecification` node (design spec §4.2).
#[derive(Debug, Clone, PartialEq)]
pub enum MemberItem {
    Member(MemberLine),
    Instance(InlineInstance),
}

/// An inline `## Members` instance: `- instance of [Classifier](./c.md) as
/// <name>[ with <n> set to <v> and …]`. Promoted to a pool node keyed
/// `{diagram}#<name>` (mirrors `build_flows`), auto-added to the diagram's members.
#[derive(Debug, Clone, PartialEq)]
pub struct InlineInstance {
    pub classifier: LinkRef,
    pub name: String,
    pub slots: Vec<ParsedSlot>,
    pub line: usize,
    pub span: Option<(usize, usize)>,
}
```

  Change `MemberGroup`:

```rust
pub struct MemberGroup {
    pub name: String,
    pub depth: u8,
    pub members: Vec<Line<MemberItem>>,
    pub children: Vec<MemberGroup>,
}
```

- [ ] **Step 5.4: Add the grammar.** In `crates/waml/src/grammar.rs`, import `InlineInstance`, `MemberItem` in the `use crate::syntax::{…}` block, add the regexes, and factor the slot-value helpers.

  First refactor Task 3's value classification into shared helpers (replace the inline `if let … strip_prefix('"')` block in `parse_slot_line`, and the `match &s.value` in `render_slot_line`, with calls to these):

```rust
static INLINE_INSTANCE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^- instance of \[([^\]]+)\]\(\./(.+?)\.md\) as ([A-Za-z_][A-Za-z0-9_]*)(?: with (.+))?$").unwrap()
});
static SLOT_ASSIGN_RE: LazyLock<Regex> = LazyLock::new(|| {
    // one `<name> set to <value>` assignment, value = quoted | link | bare token,
    // with the remaining clause (after ` and `) captured for the next iteration.
    Regex::new(r#"^([A-Za-z_][A-Za-z0-9_]*) set to ("[^"]*"|\[[^\]]+\]\(\./.+?\.md\)|\S+)(?: and (.*))?$"#).unwrap()
});

/// Classify a slot value's surface form. `None` if it is not a valid value.
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

/// Render a slot value's surface form (exact inverse of `classify_slot_value`).
pub fn render_slot_value(v: &SlotValue) -> String {
    match v {
        SlotValue::Quoted(s) => format!("\"{s}\""),
        SlotValue::Bare(s) => s.clone(),
        SlotValue::Link(l) => format!("[{}](./{}.md)", l.title, l.slug),
    }
}

/// Parse a `<n> set to <v> and <n2> set to <v2> …` clause into ordered slots.
fn parse_slot_clause(clause: &str, whole: &str) -> Result<Vec<ParsedSlot>, LineError> {
    let err = || LineError {
        range: bullet_range(whole),
        message: "malformed instance slot clause — expected '<name> set to <value>[ and …]'".to_string(),
    };
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

/// Parse `- instance of [Classifier](./c.md) as <name>[ with <clause>]`.
pub fn parse_inline_instance(line: &str) -> Result<InlineInstance, LineError> {
    let err = || LineError {
        range: bullet_range(line),
        message: "malformed inline instance — expected '- instance of [Title](./slug.md) as <name>[ with <n> set to <v> and …]'".to_string(),
    };
    let trimmed = line.trim_end_matches('\r').trim();
    let caps = INLINE_INSTANCE_RE.captures(trimmed).ok_or_else(err)?;
    let classifier = LinkRef { title: caps[1].to_string(), slug: caps[2].to_string() };
    let name = caps[3].to_string();
    let slots = match caps.get(4) {
        Some(clause) => parse_slot_clause(clause.as_str(), trimmed)?,
        None => Vec::new(),
    };
    Ok(InlineInstance { classifier, name, slots, line: 0, span: None })
}

/// Exact inverse of `parse_inline_instance` (canonical ` and `-joined clause).
pub fn render_inline_instance(i: &InlineInstance) -> String {
    let mut s = format!("- instance of [{}](./{}.md) as {}", i.classifier.title, i.classifier.slug, i.name);
    if !i.slots.is_empty() {
        let clause = i
            .slots
            .iter()
            .map(|sl| format!("{} set to {}", sl.name, render_slot_value(&sl.value)))
            .collect::<Vec<_>>()
            .join(" and ");
        s.push_str(&format!(" with {clause}"));
    }
    s
}
```

  Then update `parse_slot_line` (Task 3) to `let value = classify_slot_value(caps[2].trim()).ok_or_else(err)?;` and `render_slot_line` to `let v = render_slot_value(&s.value);`.

- [ ] **Step 5.5: Teach `parse_members_block` the inline item.** In `parse_members_block`, replace the `let node = match parse_member_line(raw) { … }` block with:

```rust
        let line_no = crate::parse::line_at(src, content_abs_start + line_start);
        let node = match parse_member_line(raw) {
            Ok(mut m) => {
                m.line = line_no;
                m.span = Some(crate::parse::find_link_span(raw, &m.title, &m.slug));
                Line::Parsed(MemberItem::Member(m))
            }
            Err(_) => match parse_inline_instance(raw) {
                Ok(mut inst) => {
                    inst.line = line_no;
                    inst.span = Some(crate::parse::find_link_span(raw, &inst.classifier.title, &inst.classifier.slug));
                    Line::Parsed(MemberItem::Instance(inst))
                }
                Err(_) => Line::Error(ErrorNode {
                    raw: raw.to_string(),
                    line: line_no,
                    span: bullet_range(raw),
                    code: DiagCode::DroppableContent,
                    message: crate::parse::DROPPABLE_MSG.to_string(),
                }),
            },
        };
```

- [ ] **Step 5.6: Teach `render_members_block` the inline item.** In `render_members_block`'s `render_group`, replace the `match m { Line::Parsed(ml) => render_member_line(ml), … }` with:

```rust
            match m {
                crate::syntax::Line::Parsed(crate::syntax::MemberItem::Member(ml)) => out.push_str(&render_member_line(ml)),
                crate::syntax::Line::Parsed(crate::syntax::MemberItem::Instance(i)) => out.push_str(&render_inline_instance(i)),
                crate::syntax::Line::Error(e) => out.push_str(&e.raw),
            }
```

- [ ] **Step 5.7: Run the grammar round-trip test, verify green.** Run:
  ```
  cargo test -p waml --lib inline_instance
  cargo test -p waml --lib slot_
  ```
  Expected: PASS (the factored `classify_slot_value`/`render_slot_value` keep Task 3's slot tests green).

### Phase B — promotion in parse (TDD)

- [ ] **Step 5.8: Write the failing promotion + serde tests.** In `crates/waml/tests/serde_shape.rs` add:

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
    // Promoted pool node keyed {diagram}#name.
    let inst = v["nodes"].as_array().unwrap().iter().find(|n| n["key"] == "m/objects#order42").unwrap();
    assert_eq!(inst["type"], "uml.InstanceSpecification");
    assert_eq!(inst["slots"][0]["value"], "ORD-42");
    assert_eq!(inst["slots"][1]["value"], "PLACED");
    // InstanceOf edge to the classifier.
    let io = v["edges"].as_array().unwrap().iter()
        .find(|e| e["kind"] == "instanceof" && e["from"] == "m/objects#order42").unwrap();
    assert_eq!(io["to"], "m/order");
    // Auto-added to the diagram's members.
    let members = &v["diagrams"][0]["groups"][0]["members"];
    assert!(members.as_array().unwrap().iter().any(|k| k == "m/objects#order42"));
}
```

  Also add a serialize round-trip test in `crates/waml/src/serialize.rs`:

```rust
    #[test]
    fn serialize_round_trips_inline_instance_member() {
        let text = "---\ntype: Diagram\ntitle: Objects\nprofile: uml-domain\n---\n# Objects\n\n## Members\n- [Order](./order.md)\n- instance of [Order](./order.md) as order42 with id set to \"ORD-42\" and status set to PLACED\n";
        let (doc, _) = crate::parse::parse(text);
        assert_eq!(crate::serialize::serialize_document(&doc), text, "inline instance member must round-trip byte-identically");
    }
```

- [ ] **Step 5.9: Run them, verify they fail.** Run:
  ```
  cargo test -p waml --features serde --test serde_shape inline_instance_is_promoted
  ```
  Expected: FAIL — the promoted node/edge/member do not exist yet (the diagram ignores the inline line).

- [ ] **Step 5.10: Thread the diagram key + promotion through `build_diagrams`.** In `crates/waml/src/parse.rs`:
  - Change `fn resolve_group` to take the diagram key and resolve both item kinds:

```rust
fn resolve_group(
    g: &crate::syntax::MemberGroup,
    referring_path: &str,
    diagram_key: &str,
    keyset: &HashSet<&str>,
) -> DiagramGroup {
    use crate::syntax::MemberItem;
    DiagramGroup {
        name: g.name.clone(),
        members: g
            .members
            .iter()
            .filter_map(Line::parsed)
            .filter_map(|item| match item {
                MemberItem::Member(m) => {
                    let resolved = crate::okf::resolve_href(referring_path, &m.slug);
                    keyset.contains(resolved.as_str()).then_some(resolved)
                }
                // Promoted inline instance: its pool key always exists (we create it).
                MemberItem::Instance(inst) => Some(format!("{diagram_key}#{}", inst.name)),
            })
            .collect(),
        children: g
            .children
            .iter()
            .map(|c| resolve_group(c, referring_path, diagram_key, keyset))
            .collect(),
    }
}
```

  - Add a recursive promoter (mirrors `build_flows`' node/edge pooling):

```rust
fn promote_inline_instances(
    g: &crate::syntax::MemberGroup,
    p: &ParsedDoc,
    diagram_key: &str,
    keyset: &HashSet<&str>,
    nodes: &mut Vec<Node>,
    edges: &mut Vec<Edge>,
) {
    use crate::syntax::MemberItem;
    for item in g.members.iter().filter_map(Line::parsed) {
        let MemberItem::Instance(inst) = item else { continue };
        let node_key = format!("{diagram_key}#{}", inst.name);
        let mut concept = crate::okf::project(&format!("{node_key}.md"), &format!("# {}\n", inst.name));
        concept.title = Some(inst.name.clone());
        nodes.push(Node {
            concept,
            key: node_key.clone(),
            ty: ElementType::Uml(crate::model::UmlMetaclass::InstanceSpecification),
            stereotypes: vec![],
            abstract_: false,
            attributes: vec![],
            values: vec![],
            note_body: None,
            annotates: vec![],
            members: vec![],
            slots: inst.slots.iter().map(|s| resolve_slot(s, &p.path, keyset)).collect(),
        });
        // `instance of` edge to the resolved classifier (skipped if unresolved;
        // validate warns in Task 6).
        let target = crate::okf::resolve_href(&p.path, &inst.classifier.slug);
        if keyset.contains(target.as_str()) {
            edges.push(Edge {
                source: node_key.clone(),
                target,
                kind: crate::model::RelationshipKind::InstanceOf,
                name: None,
                from_end: crate::model::RelEnd::default(),
                to_end: crate::model::RelEnd::default(),
                bidirectional: false,
            });
        }
    }
    for c in &g.children {
        promote_inline_instances(c, p, diagram_key, keyset, nodes, edges);
    }
}
```

  - Change `build_diagrams` to return the promoted pools and call both functions per members block:

```rust
fn build_diagrams(parsed: &[ParsedDoc], keyset: &HashSet<&str>) -> (Vec<Diagram>, Vec<Node>, Vec<Edge>) {
    let mut out = Vec::new();
    let mut inst_nodes: Vec<Node> = Vec::new();
    let mut inst_edges: Vec<Edge> = Vec::new();
    for p in parsed.iter().filter(|p| p.ty == ElementType::Diagram) {
        // … existing title/profile/description/display setup unchanged …
        let diagram_key = p.id.clone();
        let mut groups = Vec::new();
        let mut layout = Vec::new();
        for s in &p.doc.sections {
            match s {
                Section::Members(block) => {
                    groups = block
                        .groups
                        .iter()
                        .map(|g| resolve_group(g, &p.path, &diagram_key, keyset))
                        .collect();
                    for g in &block.groups {
                        promote_inline_instances(g, p, &diagram_key, keyset, &mut inst_nodes, &mut inst_edges);
                    }
                }
                Section::Layout(items) => { /* unchanged */ }
                _ => {}
            }
        }
        // … push Diagram { … } unchanged …
    }
    (out, inst_nodes, inst_edges)
}
```

  Ensure `Edge`, `RelEnd`, `UmlMetaclass`, `RelationshipKind` are imported in `parse.rs` (add to the `use crate::model::{…}` block, or fully-qualify as above). `resolve_slot` is the Task-3 helper.

- [ ] **Step 5.11: Merge the promoted pools in `build_model`.** In `fn build_model`, change the diagram/nodes/edges wiring:

```rust
    let mut nodes: Vec<Node> = classifiers.iter().map(|p| build_node(p, &keyset)).collect();
    let mut edges: Vec<Edge> = build_edges(&classifiers, &keyset);
    let (diagrams, inst_nodes, inst_edges) = build_diagrams(&parsed, &keyset);
    nodes.extend(inst_nodes);
    edges.extend(inst_edges);
```

  (Leave the rest of `build_model` — packages, flows, interactions, the returned `Model { … }` — unchanged.)

- [ ] **Step 5.12: Adapt `validate.rs::check_group_members`.** In `crates/waml/src/validate.rs`, change the member loop to match `MemberItem::Member` (skip `Instance` — its validation is Task 6):

```rust
    use crate::syntax::MemberItem;
    for item in g.members.iter().filter_map(Line::parsed) {
        let MemberItem::Member(m) = item else { continue };
        let resolved = crate::okf::resolve_href(path, &m.slug);
        if !keyset.contains(&resolved) {
            // … existing UnresolvedTarget warn, unchanged …
        }
    }
```

  Also fix any other site the compiler flags where `g.members` was assumed to be `Line<MemberLine>` (grep `g.members` / `.members.iter()` in `validate.rs`, `parse.rs`, and tests). `push_line_errors` is generic over `Line<T>`, so `push_group_errors` needs no change.

- [ ] **Step 5.13: Run the full Rust gate, verify green.** Run:
  ```
  cargo test --workspace
  cargo clippy -p waml --all-targets
  cargo fmt
  ```
  Expected: all green, including the promotion + serialize round-trip tests and every existing members/diagram/validate test (a plain `- [Title](./slug.md)` member still resolves identically through the `MemberItem::Member` arm).

- [ ] **Step 5.14: Confirm the binding is unchanged.** Run:
  ```
  pnpm build:wasm
  git diff --exit-code packages/wasm/src/generated/waml_wasm.d.ts
  pnpm lint && pnpm build && pnpm -r test
  ```
  Expected: NO binding diff (promoted nodes/edges use existing wire types); TS gate green.

- [ ] **Step 5.15: Commit.** Run:
  ```
  git add crates/waml/src/syntax.rs crates/waml/src/grammar.rs crates/waml/src/parse.rs crates/waml/src/validate.rs crates/waml/src/serialize.rs crates/waml/tests/serde_shape.rs
  git commit -F - <<'EOF'
  feat(waml): promote inline diagram instances to pool nodes with edges

  Plan: instances-object-diagrams
  Plan-Tasks: Task 5
  EOF
  ```
