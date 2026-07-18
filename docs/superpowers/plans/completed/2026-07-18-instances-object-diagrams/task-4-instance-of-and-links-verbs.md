# Instances + Object Diagrams — Task 4: `instance of` and `links` relationship verbs

> **Segment 4 of 6** of the **Instances + Object Diagrams** plan (slug `instances-object-diagrams`). See [`README.md`](README.md) for Goal, Global Constraints, File Structure, and the full plan preserved verbatim as [`_source.md`](_source.md).
> **REQUIRED SUB-SKILL:** superpowers:subagent-driven-development (recommended) or superpowers:executing-plans. Steps use checkbox (`- [ ]`) syntax for tracking.

### Task 4: instance-of and links relationship verbs (parse/serialize + edges)

Teach the `## Relationships` grammar the two new verbs so a standalone instance doc's `- instance of [Classifier]` and `- links [Instance] as [Assoc]` bullets parse and round-trip, and confirm they flow through the existing `build_edges` into `Model.edges` as `InstanceOf` / `Links` edges. **Rust-only, no binding change** (the `RelationshipKind` variants landed in Task 2) — independently green.

**Guardrails (from the design spec):**
- `instance of` → `InstanceOf` edge: `source` = instance key, `target` = classifier key. No `as` name, no ends.
- `links` → `Links` edge: `source`/`target` = instance keys; the optional `as [Assoc]` names the instantiated Association, carried by the **existing** `Edge.name: Option<AssocName>` (`AssocName::Assoc` when it resolves). No ends.
- Both are ordinary `Edge`s in `Model.edges`. NO separate pool, NO separate type, NO new `Edge` field.
- `build_edges` already handles every non-`Associates` kind generically — do NOT special-case these in `build_edges`; they must fall through the existing dedup/`else` path. (Verify by test.)
- Validation of these verbs (unresolved / non-classifier targets) is **Task 6** — do NOT touch `validate.rs` here.
- `Lifeline.ref_` is untouched — this task adds NO sequence/lifeline handling.

**Files:**
- Modify: `crates/waml/src/grammar.rs` — extend `REL_RE`'s verb alternation with `instance of|links`; extend the `rel_error_message` hint if it enumerates verbs.
- Test: `crates/waml/src/grammar.rs` (round-trip), `crates/waml/src/parse.rs` (build_edges), `crates/waml/tests/serde_shape.rs` (edge wire shape).

**Interfaces:**
- Produces: `parse_relationship_line("- instance of [Order](./order.md)")` → `ParsedRel { kind: InstanceOf, name: None, .. }`; `parse_relationship_line("- links [l](./l.md) as [A](./a.md)")` → `ParsedRel { kind: Links, name: Some(ParsedName::Ref{ title: "A", slug: "a" }), .. }`; `render_relationship_line` is the exact inverse (already generic over `kind.as_str()`).
- Consumes: `RelationshipKind::{InstanceOf, Links}` and their `parse`/`as_str` arms (Task 2), the generic `build_edges` (unchanged).

- [ ] **Step 4.1: Write the failing grammar round-trip + build tests.** In `crates/waml/src/grammar.rs` tests add:

```rust
    #[test]
    fn instance_of_and_links_relationships_round_trip() {
        for line in [
            "- instance of [Order](./order.md)",
            "- links [order42-line](./order42-line.md) as [Order→OrderLine](./order-orderline-assoc.md)",
        ] {
            let r = parse_relationship_line(line).unwrap();
            assert_eq!(render_relationship_line(&r), line, "must round-trip byte-identically");
        }
        assert_eq!(parse_relationship_line("- instance of [Order](./order.md)").unwrap().kind, RelationshipKind::InstanceOf);
        let links = parse_relationship_line("- links [l](./l.md) as [A](./a.md)").unwrap();
        assert_eq!(links.kind, RelationshipKind::Links);
        assert!(matches!(links.name, Some(crate::syntax::ParsedName::Ref { .. })));
    }
```

  In `crates/waml/src/parse.rs` tests add:

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

- [ ] **Step 4.2: Run them, verify they fail.** Run:
  ```
  cargo test -p waml --lib instance_of_and_links
  ```
  Expected: FAIL — `REL_RE` does not yet match the `instance of` / `links` verbs, so `parse_relationship_line` errors and no edges are produced.

- [ ] **Step 4.3: Extend `REL_RE`.** In `crates/waml/src/grammar.rs`, in the `REL_RE` verb alternation, append `|instance of|links` so the first group reads:

```rust
        r"^- (associates|aggregates|composes|specializes|implements|depends|includes|extends|instance of|links) ",
```

  (Everything else in `REL_RE` — the `[Title](./slug.md)` target, the optional `as` name, the optional ends — is unchanged and already correct for both verbs: neither takes ends, and `links … as [Assoc]` reuses the name-link capture groups.)

- [ ] **Step 4.4: Extend `rel_error_message` if it enumerates verbs.** Open `fn rel_error_message` (near `parse_relationship_line`). If its hint text lists the accepted verbs, add `instance of` / `links`; if it is generic, leave it. Do NOT change its structure.

- [ ] **Step 4.5: Run the full Rust gate, verify green.** Run:
  ```
  cargo test --workspace
  cargo clippy -p waml --all-targets
  cargo fmt
  ```
  Expected: all green — the two new tests pass; `build_edges` needed no change (the verbs fall through the generic non-`Associates` path); existing relationship round-trip/validate tests unaffected.

- [ ] **Step 4.6: Confirm the binding is unchanged.** Run:
  ```
  pnpm build:wasm
  git diff --exit-code packages/wasm/src/generated/waml_wasm.d.ts
  pnpm lint && pnpm build && pnpm -r test
  ```
  Expected: NO binding diff (no new tsify type this task); TS gate green.

- [ ] **Step 4.7: Commit.** Run:
  ```
  git add crates/waml/src/grammar.rs crates/waml/src/parse.rs
  git commit -F - <<'EOF'
  feat(waml): parse instance-of and links relationship verbs into edges

  Plan: instances-object-diagrams
  Plan-Tasks: Task 4
  EOF
  ```
