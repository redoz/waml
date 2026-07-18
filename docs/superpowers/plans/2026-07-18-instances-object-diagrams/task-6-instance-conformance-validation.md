# Instances + Object Diagrams — Task 6: Instance conformance validation

> **Segment 6 of 6** of the **Instances + Object Diagrams** plan (slug `instances-object-diagrams`). See [`README.md`](README.md) for Goal, Global Constraints, File Structure, and the full plan preserved verbatim as [`_source.md`](_source.md).
> **REQUIRED SUB-SKILL:** superpowers:subagent-driven-development (recommended) or superpowers:executing-plans. Steps use checkbox (`- [ ]`) syntax for tracking.

### Task 6: Instance conformance validation (three warn-only diagnostics)

Add the three **warn-only** conformance diagnostics for instances, matching waml's tolerant `Unknown`-type posture (object diagrams are often sketched before their classifiers settle). This adds `DiagCode` variants (a tsify enum → the binding regenerates), so it lands Rust + the regenerated binding in ONE green commit — the slice's second and final binding regen.

**Guardrails (from the design spec §5):**
- `SlotUnknownAttribute` — a slot name is not an attribute of the referenced classifier. **Warning.**
- `InstanceOfNonClassifier` — the `instance of` target's `is_classifier()` is `false` (includes pointing at another instance — "you do not instantiate an instance"). **Warning.**
- `InstanceOfUnresolved` — the classifier ref is dangling. **Warning.**
- All three are `Diagnostic::warn(...)` (Warning severity), never Errors. Also list them in `DiagCode::severity()`'s Warning arm.
- These validate BOTH the standalone instance doc (`## Relationships` `instance of` + `## Slots`) AND the inline instance (a diagram's `## Members` `MemberItem::Instance`).
- The generic `## Relationships` `UnresolvedTarget` (Error) loop must SKIP `InstanceOf`/`Links` kinds — instance-of resolution is reported by the warn-only `InstanceOfUnresolved`, and `links` is tolerant (no diagnostic when dangling). Do NOT let an unresolved `instance of` surface as a hard `UnresolvedTarget` Error.
- Do NOT hand-edit generated files.

**Files:**
- Modify: `crates/waml/src/diagnostic.rs` — add three `DiagCode` variants + `as_str` arms + `severity` Warning arm; extend the `as_str` round-trip test.
- Modify: `crates/waml/src/validate.rs` — build a `docs_by_key` + attribute-name lookup; add the instance-conformance pass; skip `InstanceOf`/`Links` in the generic relationship loop.
- Modify (regenerated, do NOT hand-edit): `packages/wasm/src/generated/waml_wasm.d.ts` (+ `.js`) — via `pnpm build:wasm`.
- Test: `crates/waml/src/validate.rs` (inline tests, one per code, plus a clean-instance no-warn test).

**Interfaces:**
- Produces: `DiagCode::{SlotUnknownAttribute, InstanceOfNonClassifier, InstanceOfUnresolved}` (kebab-case wire values `slot-unknown-attribute` / `instance-of-non-classifier` / `instance-of-unresolved`), all `Severity::Warning`.
- Consumes: `ElementType::is_classifier` (Task 1), `RelationshipKind::{InstanceOf, Links}` (Task 2), `Section::Slots`/`SlotValue` (Task 3), `MemberItem::Instance`/`InlineInstance` (Task 5), the parsed `Document`s passed to `link`.

### Phase A — DiagCodes (TDD)

- [ ] **Step 6.1: Write the failing validation tests.** In `crates/waml/src/validate.rs` (inline `#[cfg(test)] mod tests`), add. (These call `validate(&bundle)` — the public entry that parses + links.)

```rust
    #[test]
    fn instance_of_unresolved_classifier_warns() {
        let b = vec![(
            "m/order42.md".into(),
            "---\ntype: uml.InstanceSpecification\ntitle: order42\n---\n# order42\n\n## Relationships\n- instance of [Gone](./gone.md)\n".into(),
        )];
        let d = crate::validate::validate(&b);
        let w = d.iter().find(|x| x.code == DiagCode::InstanceOfUnresolved).unwrap();
        assert_eq!(w.severity, crate::diagnostic::Severity::Warning);
        assert!(d.iter().all(|x| x.code != DiagCode::UnresolvedTarget), "instance-of must NOT surface as a hard UnresolvedTarget");
    }

    #[test]
    fn instance_of_non_classifier_target_warns() {
        let b = vec![
            ("m/order42.md".into(), "---\ntype: uml.InstanceSpecification\ntitle: order42\n---\n# order42\n\n## Relationships\n- instance of [line42](./line42.md)\n".into()),
            ("m/line42.md".into(), "---\ntype: uml.InstanceSpecification\ntitle: line42\n---\n# line42\n".into()),
        ];
        let d = crate::validate::validate(&b);
        assert!(d.iter().any(|x| x.code == DiagCode::InstanceOfNonClassifier && x.severity == crate::diagnostic::Severity::Warning));
    }

    #[test]
    fn slot_unknown_attribute_warns() {
        let b = vec![
            ("m/order.md".into(), "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n\n## Attributes\n- id: OrderId {1}\n".into()),
            ("m/order42.md".into(), "---\ntype: uml.InstanceSpecification\ntitle: order42\n---\n# order42\n\n## Relationships\n- instance of [Order](./order.md)\n\n## Slots\n- id: \"ORD-42\"\n- bogus: 3\n".into()),
        ];
        let d = crate::validate::validate(&b);
        let w: Vec<_> = d.iter().filter(|x| x.code == DiagCode::SlotUnknownAttribute).collect();
        assert_eq!(w.len(), 1, "only the unknown slot 'bogus' warns; 'id' is a known attribute");
        assert_eq!(w[0].severity, crate::diagnostic::Severity::Warning);
    }

    #[test]
    fn conformant_instance_produces_no_instance_warnings() {
        let b = vec![
            ("m/order.md".into(), "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n\n## Attributes\n- id: OrderId {1}\n- status: Status {1}\n".into()),
            ("m/order42.md".into(), "---\ntype: uml.InstanceSpecification\ntitle: order42\n---\n# order42\n\n## Relationships\n- instance of [Order](./order.md)\n\n## Slots\n- id: \"ORD-42\"\n- status: PLACED\n".into()),
        ];
        let d = crate::validate::validate(&b);
        assert!(d.iter().all(|x| !matches!(x.code,
            DiagCode::SlotUnknownAttribute | DiagCode::InstanceOfNonClassifier | DiagCode::InstanceOfUnresolved)),
            "a conformant instance must emit no instance-conformance diagnostics: {d:?}");
    }
```

- [ ] **Step 6.2: Run them, verify they fail to compile.** Run:
  ```
  cargo test -p waml --lib instance_of_unresolved
  ```
  Expected: FAIL to compile — the three `DiagCode` variants do not exist.

- [ ] **Step 6.3: Add the `DiagCode` variants.** In `crates/waml/src/diagnostic.rs`:
  - Add to `pub enum DiagCode` (after `MalformedLifeline`):
    ```rust
        SlotUnknownAttribute,
        InstanceOfNonClassifier,
        InstanceOfUnresolved,
    ```
  - Add to `as_str` (exhaustive — no catch-all):
    ```rust
            DiagCode::SlotUnknownAttribute => "slot-unknown-attribute",
            DiagCode::InstanceOfNonClassifier => "instance-of-non-classifier",
            DiagCode::InstanceOfUnresolved => "instance-of-unresolved",
    ```
  - Add all three to `severity()`'s Warning arm:
    ```rust
            DiagCode::UnknownType
            | DiagCode::UnresolvedLayoutRef
            | DiagCode::SlotUnknownAttribute
            | DiagCode::InstanceOfNonClassifier
            | DiagCode::InstanceOfUnresolved => Severity::Warning,
    ```

### Phase B — the validation pass

- [ ] **Step 6.4: Add a free helper for the instance-of target.** In `crates/waml/src/validate.rs`, add near the other free helpers:

```rust
/// Warn (never error) on an `instance of` target: unresolved → `InstanceOfUnresolved`;
/// resolved but not a classifier (incl. another instance) → `InstanceOfNonClassifier`.
fn check_instance_of_target(
    target_slug: &str,
    resolved: &str,
    in_keyset: bool,
    target_ty: Option<&ElementType>,
    path: &str,
    line: usize,
    span: Option<(usize, usize)>,
    diags: &mut Vec<Diagnostic>,
) {
    let push = |code: DiagCode, msg: String, diags: &mut Vec<Diagnostic>| {
        let mut d = Diagnostic::warn(code, msg, path, line);
        if let Some(sp) = span {
            d = d.with_span(sp);
        }
        diags.push(d);
    };
    if !in_keyset {
        push(
            DiagCode::InstanceOfUnresolved,
            format!("'instance of' target './{target_slug}.md' resolves to no document"),
            diags,
        );
    } else if !target_ty.map(ElementType::is_classifier).unwrap_or(false) {
        push(
            DiagCode::InstanceOfNonClassifier,
            format!("'instance of' target '{resolved}' is not a classifier — you do not instantiate an instance"),
            diags,
        );
    }
}
```

- [ ] **Step 6.5: Build the attribute-name lookup + skip instance kinds in the generic loop.** In `link`, after `types` is built, add:

```rust
    let docs_by_key: HashMap<String, &crate::syntax::Document> =
        docs.iter().map(|(p, _, d)| (crate::okf::id_of(p), d)).collect();
    // Attribute names declared by a (classifier) document, for slot conformance.
    let attr_names_of = |key: &str| -> Option<HashSet<String>> {
        let doc = docs_by_key.get(key)?;
        let mut names = HashSet::new();
        for s in &doc.sections {
            if let Section::Attributes(a) = s {
                for at in a.iter().filter_map(Line::parsed) {
                    names.insert(at.name.clone());
                }
            }
        }
        Some(names)
    };
```

  In the existing `Section::Relationships(rels)` arm, at the top of the `for r in …` body, add:

```rust
                        // Instance-authored edges get warn-only conformance
                        // checks below — never the hard UnresolvedTarget Error.
                        if matches!(r.kind, RelationshipKind::InstanceOf | RelationshipKind::Links) {
                            continue;
                        }
```

- [ ] **Step 6.6: Add the instance-conformance pass.** In `link`, inside the `for (path, ty, doc) in docs` loop, AFTER the `for s in &doc.sections { match s { … } }` block, add:

```rust
        // ── Instance conformance (design spec §5), all warn-only ──────────
        if *ty == ElementType::Uml(UmlMetaclass::InstanceSpecification) {
            // Standalone instance doc: `instance of` targets + `## Slots`.
            let mut classifier: Option<String> = None;
            for s in &doc.sections {
                let Section::Relationships(rels) = s else { continue };
                for r in rels.iter().filter_map(Line::parsed) {
                    if r.kind != RelationshipKind::InstanceOf {
                        continue;
                    }
                    let resolved = crate::okf::resolve_href(path, &r.target_slug);
                    let in_keyset = keyset.contains(&resolved);
                    check_instance_of_target(
                        &r.target_slug, &resolved, in_keyset, types.get(&resolved),
                        path, r.line, r.span, &mut diags,
                    );
                    if in_keyset && types.get(&resolved).map(ElementType::is_classifier).unwrap_or(false) {
                        classifier.get_or_insert(resolved);
                    }
                }
            }
            if let Some(ck) = &classifier {
                if let Some(names) = attr_names_of(ck) {
                    for s in &doc.sections {
                        let Section::Slots(slots) = s else { continue };
                        for sl in slots.iter().filter_map(Line::parsed) {
                            if !names.contains(&sl.name) {
                                diags.push(Diagnostic::warn(
                                    DiagCode::SlotUnknownAttribute,
                                    format!("slot '{}' is not an attribute of classifier '{ck}'", sl.name),
                                    path, sl.line,
                                ));
                            }
                        }
                    }
                }
            }
        }
        if *ty == ElementType::Diagram {
            // Inline instances promoted from a diagram's `## Members`.
            use crate::syntax::MemberItem;
            fn walk_inline(
                g: &crate::syntax::MemberGroup,
                path: &str,
                keyset: &HashSet<String>,
                types: &HashMap<String, ElementType>,
                attr_names_of: &dyn Fn(&str) -> Option<HashSet<String>>,
                diags: &mut Vec<Diagnostic>,
            ) {
                for item in g.members.iter().filter_map(Line::parsed) {
                    let MemberItem::Instance(inst) = item else { continue };
                    let resolved = crate::okf::resolve_href(path, &inst.classifier.slug);
                    let in_keyset = keyset.contains(&resolved);
                    check_instance_of_target(
                        &inst.classifier.slug, &resolved, in_keyset, types.get(&resolved),
                        path, inst.line, inst.span, diags,
                    );
                    if in_keyset && types.get(&resolved).map(ElementType::is_classifier).unwrap_or(false) {
                        if let Some(names) = attr_names_of(&resolved) {
                            for sl in &inst.slots {
                                if !names.contains(&sl.name) {
                                    diags.push(Diagnostic::warn(
                                        DiagCode::SlotUnknownAttribute,
                                        format!("slot '{}' is not an attribute of classifier '{resolved}'", sl.name),
                                        path, inst.line,
                                    ));
                                }
                            }
                        }
                    }
                }
                for c in &g.children {
                    walk_inline(c, path, keyset, types, attr_names_of, diags);
                }
            }
            for s in &doc.sections {
                if let Section::Members(block) = s {
                    for g in &block.groups {
                        walk_inline(g, path, &keyset, &types, &attr_names_of, &mut diags);
                    }
                }
            }
        }
```

  (`keyset` and `types` are already in scope in `link`. `attr_names_of` is the closure from Step 6.5 — passing it as `&dyn Fn` to `walk_inline` avoids re-capturing.)

- [ ] **Step 6.7: Run the Rust gate, verify green.** Run:
  ```
  cargo test --workspace
  cargo clippy -p waml --all-targets
  cargo fmt
  ```
  Expected: all green — the four new validation tests pass; existing validation tests unaffected (non-instance docs never enter the new pass; the generic relationship loop still errors on unresolved class relationships, only `InstanceOf`/`Links` are skipped).

### Phase C — regen binding + full gate + commit

- [ ] **Step 6.8: Regenerate the binding.** Run:
  ```
  pnpm build:wasm
  git diff packages/wasm/src/generated/waml_wasm.d.ts
  ```
  Expected: the `DiagCode` union gains `"slot-unknown-attribute" | "instance-of-non-classifier" | "instance-of-unresolved"`. Do NOT hand-edit.

- [ ] **Step 6.9: Run the rest of the gate, verify green.** Run in order:
  ```
  pnpm lint
  pnpm build
  pnpm -r test
  ```
  Expected: all green — `DiagCode` is a string union; widening it type-checks (no exhaustive TS switch over it).

- [ ] **Step 6.10: Commit.** Run:
  ```
  git add crates/waml/src/diagnostic.rs crates/waml/src/validate.rs packages/wasm/src/generated/waml_wasm.d.ts packages/wasm/src/generated/waml_wasm.js
  git commit -F - <<'EOF'
  feat(waml): warn-only instance conformance validation

  Plan: instances-object-diagrams
  Plan-Tasks: Task 6
  EOF
  ```
  (Add any other `packages/wasm/src/generated/*` files `pnpm build:wasm` regenerated — e.g. `wasm-inline` — as reported by `git status`.)
