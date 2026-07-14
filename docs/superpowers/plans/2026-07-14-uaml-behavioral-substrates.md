# UAML Behavioral Substrates Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Extend UAML from structural-only (class diagrams) to the four remaining core UML families — use case, activity, state machine, and sequence — via two new structure-tier metaclasses (`uml.Actor`, `uml.UseCase` with `includes`/`extends` verbs) and two new self-rendering behavior substrates (**flow**: one directed graph per `uml.Activity`/`uml.StateMachine` document; **interaction**: ordered `## Lifelines` + `## Messages` per `uml.Sequence` document).

**Architecture:** The Rust `waml` crate is the single parser/model source of truth (the web app derives its graph via wasm `build_model`; the LSP publishes `validate()` diagnostics; `fmt` is `parse→serialize`). Structure additions slot into the existing closed-metaclass/verb tables (`model.rs`, `grammar.rs`, node-renderer registry). Behavior documents get a new `ClassifierType::Behavior(BehaviorKind)` dispatch arm so they are routed to new `Model.flows` / `Model.interactions` collections instead of becoming classifier nodes; their sections (`## Nodes`, `## Lifelines`, `## Messages`) join the `Section` enum with the same `Line<T>`/`ErrorNode` graceful-degradation machinery, so serialization stays lossless and LSP diagnostics ride free. The web frontend renders flow docs on a second SvelteFlow instance laid out by dagre at render time (relational, no stored coordinates — the render-time solver the spec requires) and sequence docs as a deterministic pure-SVG view; both are reachable from the existing diagram switcher.

**Tech Stack:** Rust (pulldown-cmark, regex, serde, wasm-bindgen/tsify), TypeScript, Svelte 5 + @xyflow/svelte, @dagrejs/dagre, vitest, cargo test.

**Why one plan, not three:** The three substrates share load-bearing infrastructure that would otherwise be specified twice: Phase 2 introduces the `ClassifierType::Behavior` dispatch arm, the behavior-doc routing in `build_model`, the `Model` wire extension + `toModelGraph` pass-through, and the CanvasInner behavior-view branch — all of which Phase 3 consumes verbatim. Phase 1 is independent but shares the verb-table/renderer-registry idiom and the `RelationshipKind` wire type that Phases 2–3 test fixtures reuse. Each phase is independently shippable (`main` green after every task): Phase 1 alone ships use-case diagrams; Phase 2 alone ships activity + state machine; Phase 3 ships sequence. Flow precedes interaction because interaction's message grammar reuses flow's backticked-expression terminals and the behavior-doc plumbing.

## Global Constraints

(from the spec — every task implicitly includes these)

- **Closed metaclasses, open stereotypes.** New element kinds (`uml.Actor`, `uml.UseCase`, flow node keywords `initial|final|decision|merge|fork|join|object`, message verbs `calls|sends|replies|creates|destroys`, fragment keywords `alt|opt|loop`) are closed, renderer-known sets. No new open vocabulary in code.
- **Graceful degradation everywhere.** Unknown type/verb/keyword/section → preserved verbatim (`Line::Error` / `Section::Unknown`), rendered generically, never dropped; serialization stays lossless (`serialize(parse(x))` is a fixpoint).
- **Relational, never coordinates.** Flow and interaction substrates are self-rendering; layout is computed at render time (dagre / deterministic SVG layout). No coordinates stored in any document.
- **Links are the connective tissue.** `describes:`, `refines`, lifelines, `carries`, `object` typing are markdown links resolved with the existing `okf::resolve_href` mechanism; unresolvable refs degrade to `None`/bare text, never error.
- **One sentence shape:** `[fronted clauses] <verb> <target> [: <detail>]`. Three markers: bare word = keyword/local reference; `` `backtick` `` = opaque expression; `[link](path)` = cross-document reference. Guards are delimited by the word `when`, **never `[...]`**.
- **The verb fixes the adornment, the category fixes the line** (no new line logic): `includes`/`extends` are dependencies → dashed line, open arrow, `«include»`/`«extend»` label.
- **Context rule:** `associates` ends (`: <near> to <far>`) required between classifiers, **optional** on an actor↔use-case communication link.
- **`transitions` is the one edge verb for both flow flavors.** `carries` (not `as`) carries object flow. `entry:`/`do:`/`exit:` reuse the colon-detail idiom.
- **Document order is time order** in `## Messages`; `else` is a proper operand sibling of `when`, nested under its fragment.
- **Deferred (do NOT implement):** `par` operands, self/found/lost messages, gates, coregions, history pseudostates, deferred events, entry/exit points, cross-boundary transitions into `refines`d submachines, object-flow pins, object flows on class diagrams, swimlane detail beyond `partition:`, edge `as <name>` labels.

## File Structure

```
crates/waml/src/
  model.rs        # + Actor/UseCase, Includes/Extends, BehaviorKind, FlowDoc/FlowNode/FlowEdge,
                  #   SequenceDoc/Lifeline/SeqItem/SeqOperand/MessageVerb/FragmentKind, Model.flows/.interactions
  syntax.rs       # + LinkRef, FlowBlock/FlowNodeSyntax/FlowBullet/FlowTransition/FlowTargetRef,
                  #   LifelineLine, MessagesBlock/SeqItemSyntax/SeqOperandSyntax/ParsedMessage,
                  #   Section::Nodes/Lifelines/Messages
  grammar.rs      # + verb list, ends-optional associates, flow line/heading/block parse+render,
                  #   lifeline/message/messages-block parse+render, parse_link_ref
  parse.rs        # + section routing, behavior-doc filtering, build_flows, build_interactions
  serialize.rs    # + Section::Nodes/Lifelines/Messages rendering + section order
  validate.rs     # + associates-ends context rule, flow + sequence semantic checks
  diagnostic.rs   # + MalformedFlowBullet, DuplicateFlowNode, MalformedMessage, MalformedLifeline
crates/waml/tests/serde_shape.rs   # + wire-shape pins for new types
packages/okf/src/types.ts          # + TS mirrors of all new wire types
packages/okf/src/grammar.ts        # + verb list + ends-optional parity
packages/core/src/state/overlay.ts # + flows/interactions pass-through
packages/core/src/profiles/umlDomain.ts # + Actor/UseCase palette entries
packages/web/src/components/canvas/nodes/UmlActorNode.svelte     (new)
packages/web/src/components/canvas/nodes/UmlUseCaseNode.svelte   (new)
packages/web/src/components/canvas/nodes/registry.ts             # + entries
packages/web/src/components/canvas/edges.ts                      # + edgeStereotype
packages/web/src/components/canvas/RelEdge.svelte                # + dashed/label for includes/extends
packages/web/src/canvas/flowGraph.ts                             (new: dagre layout + labels)
packages/web/src/components/canvas/flow/FlowStepNode.svelte      (new)
packages/web/src/components/canvas/flow/FlowControlNode.svelte   (new)
packages/web/src/components/canvas/flow/FlowObjectNode.svelte    (new)
packages/web/src/components/canvas/flow/TransitionEdge.svelte    (new)
packages/web/src/components/canvas/flow/FlowView.svelte          (new)
packages/web/src/canvas/sequenceLayout.ts                        (new: pure layout)
packages/web/src/components/canvas/sequence/SequenceView.svelte  (new)
packages/web/src/components/canvas/CanvasInner.svelte            # + behavior-view branch + switcher entries
packages/web/src/components/inspector/RelationshipInspector.svelte # + KIND_HELP entries
docs/uaml-spec.md                                                # + behavioral substrates BNF
```

Commands used throughout (run from `C:\dev\uaml`):
- Rust tests: `cargo test -p waml`
- Rebuild wasm after Rust wire changes: `pnpm build:wasm`
- Web tests: `pnpm --filter @waml/web test` · okf tests: `pnpm --filter @waml/okf test`
- Everything: `pnpm -r test && cargo test`

---

# Phase 1 — Structure tier: `uml.Actor`, `uml.UseCase`, `includes`/`extends`

### Task 1: Rust model — new metaclasses and dependency verbs

**Files:**
- Modify: `crates/waml/src/model.rs`
- Test: inline `#[cfg(test)]` in the same file

**Interfaces:**
- Consumes: nothing new.
- Produces: `UmlMetaclass::Actor`, `UmlMetaclass::UseCase` (type tokens `"uml.Actor"`, `"uml.UseCase"`); `RelationshipKind::Includes`, `RelationshipKind::Extends` (strings `"includes"`, `"extends"`, `is_ended() == false`). Later tasks (2, 3, 5, 6) match on these exact variants/strings.

- [ ] **Step 1: Write the failing tests** — in `crates/waml/src/model.rs`, extend the existing `tests` module:

```rust
    #[test]
    fn actor_and_usecase_metaclasses_parse_and_round_trip() {
        assert_eq!(
            ClassifierType::parse("uml.Actor"),
            ClassifierType::Uml(UmlMetaclass::Actor)
        );
        assert_eq!(
            ClassifierType::parse("uml.UseCase"),
            ClassifierType::Uml(UmlMetaclass::UseCase)
        );
        assert_eq!(ClassifierType::Uml(UmlMetaclass::Actor).as_str(), "uml.Actor");
        assert_eq!(ClassifierType::Uml(UmlMetaclass::UseCase).as_str(), "uml.UseCase");
    }

    #[test]
    fn includes_and_extends_are_endless_dependency_verbs() {
        assert_eq!(RelationshipKind::parse("includes"), Some(RelationshipKind::Includes));
        assert_eq!(RelationshipKind::parse("extends"), Some(RelationshipKind::Extends));
        assert_eq!(RelationshipKind::Includes.as_str(), "includes");
        assert_eq!(RelationshipKind::Extends.as_str(), "extends");
        assert!(!RelationshipKind::Includes.is_ended());
        assert!(!RelationshipKind::Extends.is_ended());
    }
```

Also extend the loop in the existing `relationship_kind_round_trips` test with `RelationshipKind::Includes, RelationshipKind::Extends`.

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p waml model::tests`
Expected: FAIL — `no variant or associated item named 'Actor'` (compile error).

- [ ] **Step 3: Implement** — in `crates/waml/src/model.rs`:

In `enum RelationshipKind` add variants after `Depends`:

```rust
    Includes,
    Extends,
```

In `RelationshipKind::as_str` add arms:

```rust
            RelationshipKind::Includes => "includes",
            RelationshipKind::Extends => "extends",
```

In `RelationshipKind::parse` add arms:

```rust
            "includes" => Some(RelationshipKind::Includes),
            "extends" => Some(RelationshipKind::Extends),
```

(`is_ended` needs no change — the `matches!` covers only the association family.)

In `enum UmlMetaclass` add variants after `Association`:

```rust
    Actor,
    UseCase,
```

In `UmlMetaclass::parse` add arms:

```rust
            "Actor" => Some(UmlMetaclass::Actor),
            "UseCase" => Some(UmlMetaclass::UseCase),
```

In `UmlMetaclass::name` add arms:

```rust
            UmlMetaclass::Actor => "Actor",
            UmlMetaclass::UseCase => "UseCase",
```

- [ ] **Step 4: Run to verify pass**

Run: `cargo test -p waml`
Expected: PASS (all existing + new tests).

- [ ] **Step 5: Commit**

```bash
git add crates/waml/src/model.rs
git commit -m "feat(model): add uml.Actor/uml.UseCase metaclasses and includes/extends verbs"
```

---

### Task 2: Rust grammar — parse/render the new verbs; make `associates` ends optional

**Files:**
- Modify: `crates/waml/src/grammar.rs` (REL_RE, `rel_error_message`, `parse_relationship_line`, `render_relationship_line`)
- Test: inline `#[cfg(test)]` in the same file

**Interfaces:**
- Consumes: `RelationshipKind::{Includes, Extends}` from Task 1.
- Produces: `parse_relationship_line("- includes [X](./x.md)")` → `Ok(ParsedRel { kind: Includes, .. })`; `parse_relationship_line("- associates [X](./x.md)")` → `Ok` with both `RelEnd::default()` (multiplicity `None`); `render_relationship_line` renders an ends-less associates with no `:` clause. Task 3 relies on "associates with `from_end.multiplicity == None`" meaning "authored without ends".

- [ ] **Step 1: Write the failing tests** — append to `grammar.rs` `tests`:

```rust
    #[test]
    fn parses_includes_and_extends_without_ends() {
        let r = parse_relationship_line("- includes [Authenticate](./authenticate.md)").unwrap();
        assert_eq!(r.kind, RelationshipKind::Includes);
        assert_eq!(r.target_slug, "authenticate");
        let r = parse_relationship_line("- extends [Apply Coupon](./apply-coupon.md)").unwrap();
        assert_eq!(r.kind, RelationshipKind::Extends);
        assert!(parse_relationship_line("- includes [A](./a.md): 1 to 1").is_err());
    }

    #[test]
    fn associates_without_ends_parses_as_bare_communication_link() {
        let r = parse_relationship_line("- associates [Customer](./customer.md)").unwrap();
        assert_eq!(r.kind, RelationshipKind::Associates);
        assert_eq!(r.from_end, RelEnd::default());
        assert_eq!(r.to_end, RelEnd::default());
    }

    #[test]
    fn renders_endless_associates_and_use_case_verbs_round_trip() {
        for line in [
            "- associates [Customer](./customer.md)",
            "- includes [Authenticate](./authenticate.md)",
            "- extends [Apply Coupon](./apply-coupon.md)",
        ] {
            let r = parse_relationship_line(line).unwrap();
            assert_eq!(render_relationship_line(&r), line);
        }
    }
```

Also update the existing test `rejects_ends_on_forbidden_kind_and_missing_ends_on_ended`: keep both assertions (`specializes` with ends errors; `composes` without ends errors) — they remain true.

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p waml grammar::tests`
Expected: FAIL — `- includes …` yields `Err` ("unknown relationship verb"), `- associates …` without ends yields `Err`.

- [ ] **Step 3: Implement** — in `crates/waml/src/grammar.rs`:

Replace the verb alternation in `REL_RE`:

```rust
static REL_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(concat!(
        r"^- (associates|aggregates|composes|specializes|implements|depends|includes|extends) ",
        r"\[([^\]]+)\]\(\./(.+?)\.md\)",
        r#"(?: as (?:"([^"]*)"|\[([^\]]+)\]\(\./(.+?)\.md\)))?"#,
        r"(?:\s*:\s*(.+))?$",
    )).unwrap()
});
```

Replace `rel_error_message`'s constants and unknown-verb clause:

```rust
pub fn rel_error_message(line: &str) -> String {
    const ENDED: [&str; 2] = ["aggregates", "composes"];
    const OTHER: [&str; 5] = ["specializes", "implements", "depends", "includes", "extends"];
    let verb = line.trim_start_matches("- ").split_whitespace().next().unwrap_or("");
    let has_ends = has_multiplicity_ends(line);
    if ENDED.contains(&verb) && !has_ends {
        format!("'{verb}' requires ': <near> to <far>' multiplicity ends")
    } else if OTHER.contains(&verb) && has_ends {
        format!("'{verb}' does not take multiplicity ends")
    } else if verb == "annotates" {
        "note anchors ('annotates') are not supported yet".to_string()
    } else if !ENDED.contains(&verb) && !OTHER.contains(&verb) && verb != "associates" {
        format!("unknown relationship verb '{verb}'")
    } else {
        "malformed relationship line".to_string()
    }
}
```

In `parse_relationship_line`, replace the XOR ends check:

```rust
    let ends_raw = m.get(7).map(|x| x.as_str());
    // Ends: required for aggregates/composes; OPTIONAL for associates (bare =
    // actor↔use-case communication link, enforced cross-doc in validate::link);
    // forbidden for all non-ended verbs.
    match (ends_raw.is_some(), kind) {
        (true, k) if !k.is_ended() => return Err(err()),
        (false, k) if k.is_ended() && k != RelationshipKind::Associates => return Err(err()),
        _ => {}
    }
```

In `render_relationship_line`, replace the final `if`:

```rust
    let has_ends = r.from_end.multiplicity.is_some() || r.to_end.multiplicity.is_some();
    if !r.kind.is_ended() || !has_ends {
        format!("- {} {link}{name}", r.kind.as_str())
    } else {
        format!("- {} {link}{name}: {} to {}", r.kind.as_str(), render_end(&r.from_end), render_end(&r.to_end))
    }
```

- [ ] **Step 4: Run to verify pass**

Run: `cargo test -p waml`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/waml/src/grammar.rs
git commit -m "feat(grammar): parse/render includes/extends and ends-optional associates"
```

---

### Task 3: Validate — `associates` ends required between classifiers (context rule)

**Files:**
- Modify: `crates/waml/src/validate.rs` (`link`)
- Test: inline `#[cfg(test)]` in the same file

**Interfaces:**
- Consumes: Task 2's "ends-less associates parses"; `UmlMetaclass::{Actor, UseCase}` from Task 1.
- Produces: `validate(bundle)` emits an Error `DiagCode::MalformedRelationship` on an ends-less `associates` whose resolved endpoints are BOTH non-Actor/non-UseCase; silent when either endpoint is an Actor/UseCase or unresolved (graceful).

- [ ] **Step 1: Write the failing tests** — append to `validate.rs` `tests`:

```rust
    #[test]
    fn endless_associates_between_actor_and_use_case_is_clean() {
        let b = vec![
            ("u/place-order.md".into(),
             "---\ntype: uml.UseCase\ntitle: Place Order\n---\n# Place Order\n\n## Relationships\n- associates [Customer](./customer.md)\n- includes [Authenticate](./authenticate.md)\n".into()),
            ("u/customer.md".into(), "---\ntype: uml.Actor\ntitle: Customer\n---\n# Customer\n".into()),
            ("u/authenticate.md".into(), "---\ntype: uml.UseCase\ntitle: Authenticate\n---\n# Authenticate\n".into()),
        ];
        let d = validate(&b);
        assert!(d.is_empty(), "got: {d:?}");
    }

    #[test]
    fn endless_associates_between_classes_is_flagged() {
        let b = vec![
            ("c/order.md".into(),
             "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n\n## Relationships\n- associates [Customer](./customer.md)\n".into()),
            ("c/customer.md".into(), "---\ntype: uml.Class\ntitle: Customer\n---\n# Customer\n".into()),
        ];
        let d = validate(&b);
        let m = d.iter().find(|x| x.code == DiagCode::MalformedRelationship)
            .expect("classifier↔classifier associates without ends must be flagged");
        assert_eq!(m.line, 8);
    }
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p waml validate::tests`
Expected: FAIL — second test finds no diagnostic.

- [ ] **Step 3: Implement** — in `crates/waml/src/validate.rs`:

Add to the imports: `use crate::model::{ClassifierType, RelationshipKind, UmlMetaclass};` (replacing the existing `use crate::model::ClassifierType;`).

In `link`, after the `keyset`/`slug_count` loop, build a type map:

```rust
    let mut types: HashMap<String, ClassifierType> = HashMap::new();
    for (path, ty, _doc) in docs {
        types.insert(crate::okf::id_of(path), ty.clone());
    }
```

Change the second loop's binding from `for (path, _ty, doc) in docs` to `for (path, ty, doc) in docs`, and inside the `Section::Relationships` arm, after the existing unresolved-target check, add:

```rust
                        // Context rule: an ends-less `associates` is a
                        // communication link — valid only when an actor or a
                        // use case participates. Between plain classifiers,
                        // ends are required (uaml-spec.md).
                        let is_comm_party = |t: Option<&ClassifierType>| {
                            matches!(
                                t,
                                Some(ClassifierType::Uml(UmlMetaclass::Actor))
                                    | Some(ClassifierType::Uml(UmlMetaclass::UseCase))
                            )
                        };
                        if r.kind == RelationshipKind::Associates
                            && r.from_end.multiplicity.is_none()
                            && keyset.contains(&resolved)
                            && !is_comm_party(Some(ty))
                            && !is_comm_party(types.get(&resolved))
                        {
                            let mut d = Diagnostic::new(
                                DiagCode::MalformedRelationship,
                                "'associates' between classifiers requires ': <near> to <far>' multiplicity ends (ends are optional only on an actor↔use-case communication link)",
                                path,
                                r.line,
                            );
                            if let Some(span) = r.span {
                                d = d.with_span(span);
                            }
                            diags.push(d);
                        }
```

(Note: `resolved` is already computed in that arm — reuse it; keep the unresolved-target `if` as-is above this block.)

- [ ] **Step 4: Run to verify pass**

Run: `cargo test -p waml`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/waml/src/validate.rs
git commit -m "feat(validate): require associates ends between classifiers, optional on actor/use-case links"
```

---

### Task 4: TS wire parity — verb list, TS grammar, inspector help, palette

**Files:**
- Modify: `packages/okf/src/types.ts:20` (RELATIONSHIP_KINDS)
- Modify: `packages/okf/src/grammar.ts:60` (REL_RE), `parseRelationshipLine`, `renderRelationshipLine`
- Modify: `packages/web/src/components/inspector/RelationshipInspector.svelte` (KIND_HELP)
- Modify: `packages/core/src/profiles/umlDomain.ts` (palette)
- Test: `packages/okf/test/grammar.test.ts`

**Interfaces:**
- Consumes: Rust wire strings `"includes"`/`"extends"`, `"uml.Actor"`/`"uml.UseCase"` (Tasks 1–2).
- Produces: TS `RelationshipKind` union includes `"includes" | "extends"`; `UML_DOMAIN.palette.metaclasses` includes `"uml.Actor"`, `"uml.UseCase"`. Tasks 5–6 import these types.

- [ ] **Step 1: Write the failing tests** — append to `packages/okf/test/grammar.test.ts` (inside the existing top-level `describe`, alongside the `specializes takes no ends` test):

```ts
  it("parses includes and extends without ends", () => {
    expect(parseRelationshipLine("- includes [Authenticate](./authenticate.md)"))
      .toEqual({ kind: "includes", targetSlug: "authenticate", fromEnd: {}, toEnd: {} });
    expect(parseRelationshipLine("- extends [Apply Coupon](./apply-coupon.md)"))
      .toEqual({ kind: "extends", targetSlug: "apply-coupon", fromEnd: {}, toEnd: {} });
    expect(parseRelationshipLine("- includes [A](./a.md): 1 to 1")).toBeNull();
  });

  it("parses and renders an ends-less associates communication link", () => {
    expect(parseRelationshipLine("- associates [Customer](./customer.md)"))
      .toEqual({ kind: "associates", targetSlug: "customer", fromEnd: {}, toEnd: {} });
    expect(renderRelationshipLine("associates", "Customer", "customer", {}, {}))
      .toBe("- associates [Customer](./customer.md)");
    expect(renderRelationshipLine("includes", "Auth", "auth", {}, {}))
      .toBe("- includes [Auth](./auth.md)");
  });
```

- [ ] **Step 2: Run to verify failure**

Run: `pnpm --filter @waml/okf test`
Expected: FAIL — `"includes"` not assignable / `parseRelationshipLine` returns `null`.

- [ ] **Step 3: Implement**

`packages/okf/src/types.ts` line 20:

```ts
export const RELATIONSHIP_KINDS = ["associates", "aggregates", "composes", "specializes", "implements", "depends", "includes", "extends", "annotates"] as const;
```

`packages/okf/src/grammar.ts` — REL_RE verb alternation:

```ts
const REL_RE = /^- (associates|aggregates|composes|specializes|implements|depends|includes|extends) \[([^\]]+)\]\(\.\/(.+?)\.md\)(?: as (?:"([^"]*)"|\[([^\]]+)\]\(\.\/(.+?)\.md\)))?(?:\s*:\s*(.+))?$/;
```

In `parseRelationshipLine`, replace the XOR check:

```ts
  const endsRaw = m[7];
  const needsEnds = ENDED_KINDS.has(kind);
  // Ends: forbidden on non-ended verbs; required on aggregates/composes;
  // OPTIONAL on associates (bare = actor↔use-case communication link).
  if (endsRaw && !needsEnds) return null;
  if (!endsRaw && needsEnds && kind !== "associates") return null;
```

In `renderRelationshipLine`, replace the final two lines:

```ts
  const hasEnds = fromEnd.multiplicity !== undefined || toEnd.multiplicity !== undefined;
  if (!ENDED_KINDS.has(kind) || !hasEnds) return `- ${kind} ${link}${nameStr}`;
  return `- ${kind} ${link}${nameStr}: ${renderEnd(fromEnd)} to ${renderEnd(toEnd)}`;
```

`packages/web/src/components/inspector/RelationshipInspector.svelte` — add to `KIND_HELP` (it is a `Record<RelationshipKind, string>`, so the compiler forces this):

```ts
    includes: "Use-case «include» — dashed open arrow, base → included use case.",
    extends: "Use-case «extend» — dashed open arrow, extension → base use case.",
```

`packages/core/src/profiles/umlDomain.ts` — palette metaclasses:

```ts
    metaclasses: ["uml.Class", "uml.Interface", "uml.Enum", "uml.DataType", "uml.Actor", "uml.UseCase"],
```

- [ ] **Step 4: Run to verify pass**

Run: `pnpm --filter @waml/okf test && pnpm --filter @waml/web test`
Expected: PASS (web compile picks up the widened union; KIND_HELP satisfies the Record).

- [ ] **Step 5: Commit**

```bash
git add packages/okf/src/types.ts packages/okf/src/grammar.ts packages/okf/test/grammar.test.ts packages/web/src/components/inspector/RelationshipInspector.svelte packages/core/src/profiles/umlDomain.ts
git commit -m "feat(ts): includes/extends verbs, ends-optional associates, actor/use-case palette"
```

---

### Task 5: Web node renderers — actor stick figure, use-case ellipse

**Files:**
- Create: `packages/web/src/components/canvas/nodes/UmlActorNode.svelte`
- Create: `packages/web/src/components/canvas/nodes/UmlUseCaseNode.svelte`
- Modify: `packages/web/src/components/canvas/nodes/registry.ts`
- Test: `packages/web/src/components/canvas/nodes/registry.test.ts`

**Interfaces:**
- Consumes: `OkfNodeData` from `./types`, `NodePorts` component (existing).
- Produces: `resolveNodeRenderer("uml.Actor") === UmlActorNode`, `resolveNodeRenderer("uml.UseCase") === UmlUseCaseNode`.

- [ ] **Step 1: Write the failing test** — append to `registry.test.ts`:

```ts
import UmlActorNode from "./UmlActorNode.svelte";
import UmlUseCaseNode from "./UmlUseCaseNode.svelte";

it("resolves uml.Actor and uml.UseCase to their dedicated renderers", () => {
  expect(resolveNodeRenderer("uml.Actor")).toBe(UmlActorNode);
  expect(resolveNodeRenderer("uml.UseCase")).toBe(UmlUseCaseNode);
});
```

(Reuse the file's existing imports of `resolveNodeRenderer`/`expect`/`it`; add only what is missing.)

- [ ] **Step 2: Run to verify failure**

Run: `pnpm --filter @waml/web test -- registry`
Expected: FAIL — cannot resolve `./UmlActorNode.svelte`.

- [ ] **Step 3: Implement**

`packages/web/src/components/canvas/nodes/UmlActorNode.svelte`:

```svelte
<script lang="ts">
  import NodePorts from "./NodePorts.svelte";
  import type { OkfNodeData } from "./types";

  let { data }: { data: OkfNodeData } = $props();
</script>

<!-- UML Actor: stick figure with the name beneath (uaml behavioral-substrates spec). -->
<div class="relative flex w-[120px] flex-col items-center select-none">
  <NodePorts />
  <svg width="48" height="72" viewBox="0 0 48 72" class="relative z-[1]">
    <circle cx="24" cy="10" r="8" fill="#fff" stroke="#334155" stroke-width="2" />
    <line x1="24" y1="18" x2="24" y2="44" stroke="#334155" stroke-width="2" />
    <line x1="6" y1="28" x2="42" y2="28" stroke="#334155" stroke-width="2" />
    <line x1="24" y1="44" x2="8" y2="66" stroke="#334155" stroke-width="2" />
    <line x1="24" y1="44" x2="40" y2="66" stroke="#334155" stroke-width="2" />
  </svg>
  <div class="relative z-[1] mt-1 max-w-full text-center text-[12px] font-semibold text-slate-800">
    {data.concept.title ?? data.key}
  </div>
</div>
```

`packages/web/src/components/canvas/nodes/UmlUseCaseNode.svelte`:

```svelte
<script lang="ts">
  import NodePorts from "./NodePorts.svelte";
  import type { OkfNodeData } from "./types";

  let { data }: { data: OkfNodeData } = $props();
</script>

<!-- UML UseCase: ellipse with the name centered. -->
<div
  class="relative flex h-[72px] w-[190px] items-center justify-center border-[1.5px] border-[#c8d2e0] bg-white px-4 text-center shadow-[0_2px_8px_rgba(15,23,42,0.05)] select-none"
  style="border-radius: 50%;"
>
  <NodePorts />
  <div class="relative z-[1] text-[12.5px] font-semibold leading-snug text-slate-800">
    {data.concept.title ?? data.key}
  </div>
</div>
```

`registry.ts` — add imports and entries:

```ts
import UmlActorNode from "./UmlActorNode.svelte";
import UmlUseCaseNode from "./UmlUseCaseNode.svelte";
```

and inside `FAMILIES.uml`:

```ts
    Actor: UmlActorNode, // stick figure — use-case diagrams (behavioral substrates spec)
    UseCase: UmlUseCaseNode, // ellipse
```

- [ ] **Step 4: Run to verify pass**

Run: `pnpm --filter @waml/web test`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add packages/web/src/components/canvas/nodes/UmlActorNode.svelte packages/web/src/components/canvas/nodes/UmlUseCaseNode.svelte packages/web/src/components/canvas/nodes/registry.ts packages/web/src/components/canvas/nodes/registry.test.ts
git commit -m "feat(web): actor stick-figure and use-case ellipse node renderers"
```

---

### Task 6: Web edge rendering — dashed `«include»` / `«extend»` dependencies

**Files:**
- Modify: `packages/web/src/components/canvas/edges.ts` (add `edgeStereotype`)
- Modify: `packages/web/src/components/canvas/RelEdge.svelte` (DASHED set, markers, midpoint label)
- Test: `packages/web/src/components/canvas/edges.test.ts`

**Interfaces:**
- Consumes: TS `RelationshipKind` incl. `"includes" | "extends"` (Task 4).
- Produces: `edgeStereotype(kind: RelationshipKind): string | undefined` → `"«include»"` / `"«extend»"` / `undefined`.

- [ ] **Step 1: Write the failing test** — append to `edges.test.ts`:

```ts
import { edgeStereotype } from "./edges";

it("maps use-case dependency verbs to guillemet stereotypes", () => {
  expect(edgeStereotype("includes")).toBe("«include»");
  expect(edgeStereotype("extends")).toBe("«extend»");
  expect(edgeStereotype("associates")).toBeUndefined();
  expect(edgeStereotype("depends")).toBeUndefined();
});
```

- [ ] **Step 2: Run to verify failure**

Run: `pnpm --filter @waml/web test -- edges`
Expected: FAIL — `edgeStereotype` is not exported.

- [ ] **Step 3: Implement**

In `edges.ts`, add near the top (after imports; import `RelationshipKind` type from `@waml/okf`):

```ts
import type { RelationshipKind } from "@waml/okf";

// Verb → midpoint guillemet label. Category fixes the line (dashed dependency),
// the verb fixes the adornment — includes/extends add a «keyword» label.
export function edgeStereotype(kind: RelationshipKind): string | undefined {
  return kind === "includes" ? "«include»" : kind === "extends" ? "«extend»" : undefined;
}
```

In `RelEdge.svelte`:

1. Extend the dashed set (line 25):

```ts
  const DASHED: ReadonlySet<RelationshipKind> = new Set(["implements", "depends", "includes", "extends"]);
```

2. Import the helper: add `import { edgeStereotype } from "./edges";` next to the `./floating` import.

3. In `markerInfo`, change the `depends` branch condition to cover the new verbs:

```ts
    } else if (kind === "depends" || kind === "includes" || kind === "extends") {
      defs.push({ type: "arrow", key: "dep-arrow", flip: false });
      markerEnd = `url(#dep-arrow-${id})`;
    } else {
```

4. Add a derived after `labels`:

```ts
  const stereo = $derived(edgeStereotype(kind));
```

5. In the template, after the `{#each labels …}` block (before the closing `{/if}`), add:

```svelte
  {#if stereo && geometry}
    <EdgeLabel
      x={(geometry.sx + geometry.tx) / 2}
      y={(geometry.sy + geometry.ty) / 2 - 10}
      class="nodrag nopan"
      style="background:rgba(255,255,255,0.9);border-radius:4px;padding:0 4px;font-size:10.5px;font-weight:600;color:#334155;white-space:nowrap;"
    >
      {stereo}
    </EdgeLabel>
  {/if}
```

- [ ] **Step 4: Run to verify pass**

Run: `pnpm --filter @waml/web test`
Expected: PASS. Then rebuild wasm so the Phase-1 Rust changes reach the frontend: `pnpm build:wasm && pnpm --filter @waml/web test` — PASS again.

- [ ] **Step 5: Commit**

```bash
git add packages/web/src/components/canvas/edges.ts packages/web/src/components/canvas/edges.test.ts packages/web/src/components/canvas/RelEdge.svelte
git commit -m "feat(web): dashed include/extend dependency edges with guillemet labels"
```

**Phase 1 done — use-case diagrams ship:** actor/use-case docs parse, validate, curate into Diagrams (system boundary = the existing `frame` group treatment in `## Layout`), and render.

---

# Phase 2 — Flow substrate (`uml.Activity`, `uml.StateMachine`)

### Task 7: `ClassifierType::Behavior` dispatch + behavior-doc routing

**Files:**
- Modify: `crates/waml/src/model.rs` (BehaviorKind, ClassifierType)
- Modify: `crates/waml/src/parse.rs` (`build_model` classifier filter)
- Test: inline in both files

**Interfaces:**
- Consumes: nothing new.
- Produces: `BehaviorKind::{Activity, StateMachine, Sequence}` with `BehaviorKind::parse(&str) -> Option<BehaviorKind>` / `.name() -> &'static str`; `ClassifierType::Behavior(BehaviorKind)` parsing `"uml.Activity"`, `"uml.StateMachine"`, `"uml.Sequence"`; `build_model` excludes behavior docs from `Model.nodes`/`edges` keysets. Tasks 10, 11, 15, 18 match on these exact variants.

- [ ] **Step 1: Write the failing tests**

In `model.rs` `tests`:

```rust
    #[test]
    fn behavior_types_parse_and_round_trip() {
        assert_eq!(ClassifierType::parse("uml.Activity"), ClassifierType::Behavior(BehaviorKind::Activity));
        assert_eq!(ClassifierType::parse("uml.StateMachine"), ClassifierType::Behavior(BehaviorKind::StateMachine));
        assert_eq!(ClassifierType::parse("uml.Sequence"), ClassifierType::Behavior(BehaviorKind::Sequence));
        assert_eq!(ClassifierType::Behavior(BehaviorKind::StateMachine).as_str(), "uml.StateMachine");
    }
```

In `parse.rs` `model_tests`:

```rust
    #[test]
    fn behavior_docs_are_not_classifier_nodes() {
        let b = vec![
            ("m/order.md".into(), "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n".into()),
            ("m/lifecycle.md".into(), "---\ntype: uml.StateMachine\ntitle: Order Lifecycle\n---\n# Order Lifecycle\n".into()),
        ];
        let m = build_model(&b);
        assert_eq!(m.nodes.len(), 1, "a behavior doc must not become a classifier node");
        assert!(m.node("m/lifecycle").is_none());
    }
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p waml`
Expected: FAIL — `BehaviorKind` unknown (compile error).

- [ ] **Step 3: Implement**

In `model.rs`, after `UmlMetaclass`'s impl:

```rust
/// A behavior document's kind: selects the substrate (flow vs interaction) and
/// the flow flavor. Behavior docs are the document — model AND view — and are
/// never classifier nodes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BehaviorKind {
    Activity,
    StateMachine,
    Sequence,
}

impl BehaviorKind {
    pub fn parse(s: &str) -> Option<BehaviorKind> {
        match s {
            "Activity" => Some(BehaviorKind::Activity),
            "StateMachine" => Some(BehaviorKind::StateMachine),
            "Sequence" => Some(BehaviorKind::Sequence),
            _ => None,
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            BehaviorKind::Activity => "Activity",
            BehaviorKind::StateMachine => "StateMachine",
            BehaviorKind::Sequence => "Sequence",
        }
    }
}
```

Extend `ClassifierType`:

```rust
pub enum ClassifierType {
    Uml(UmlMetaclass),
    Behavior(BehaviorKind),
    Diagram,
    Unknown(String),
}
```

In `ClassifierType::parse`, inside the `family == "uml"` branch after the metaclass attempt:

```rust
                if let Some(bk) = BehaviorKind::parse(metaclass) {
                    return ClassifierType::Behavior(bk);
                }
```

In `ClassifierType::as_str`:

```rust
            ClassifierType::Behavior(bk) => format!("uml.{}", bk.name()),
```

In `parse.rs` `build_model`, change the classifier filter:

```rust
    let classifiers: Vec<&ParsedDoc> = parsed
        .iter()
        .filter(|p| {
            p.ty != ClassifierType::Diagram
                && !matches!(p.ty, ClassifierType::Behavior(_))
                && p.slug != "index"
                && p.slug != "log"
        })
        .collect();
```

Also in `model.rs`'s `model_looks_up_nodes_by_key` test, replace the `Model { … }` literal tail with `..Default::default()`:

```rust
        let model = Model { nodes: vec![node], ..Default::default() };
```

(This keeps the literal stable as `Model` grows in Tasks 10/18.)

- [ ] **Step 4: Run to verify pass**

Run: `cargo test -p waml`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/waml/src/model.rs crates/waml/src/parse.rs
git commit -m "feat(model): behavior-kind dispatch arm; behavior docs are not classifier nodes"
```

---

### Task 8: Flow syntax types + line grammar (transitions, internals, refines, partition)

**Files:**
- Modify: `crates/waml/src/model.rs` (FlowNodeKind — wire enum, used by syntax too)
- Modify: `crates/waml/src/syntax.rs` (LinkRef, FlowTransition, FlowTargetRef, FlowBullet, FlowNodeSyntax, FlowBlock)
- Modify: `crates/waml/src/grammar.rs` (parsers + renderers + `parse_link_ref`)
- Test: inline in `grammar.rs`

**Interfaces:**
- Consumes: `Line<T>`/`ErrorNode` (existing), `FlowNodeKind` (this task).
- Produces (exact signatures, used by Tasks 9–11):
  - `model::FlowNodeKind` = `Initial | Final | Decision | Merge | Fork | Join | Object | Plain` (serde lowercase).
  - `syntax::LinkRef { title: String, slug: String }`
  - `syntax::FlowTargetRef` = `Local(String) | Link(LinkRef)`
  - `syntax::FlowTransition { trigger: Option<String>, guard: Option<String>, is_else: bool, target: FlowTargetRef, carries: Option<LinkRef>, effect: Option<String>, line: usize }`
  - `syntax::FlowBullet` = `Transition(FlowTransition) | Entry(String) | Do(String) | Exit(String) | Refines(LinkRef) | Partition(String)`
  - `syntax::FlowNodeSyntax { kind: FlowNodeKind, identity: String, object_ref: Option<LinkRef>, bullets: Vec<Line<FlowBullet>>, notes: Vec<Line<String>>, line: usize }`
  - `syntax::FlowBlock { nodes: Vec<FlowNodeSyntax>, preamble_errors: Vec<ErrorNode> }`
  - `grammar::parse_flow_bullet(&str) -> Result<FlowBullet, LineError>`
  - `grammar::parse_flow_heading(&str) -> (FlowNodeKind, String, Option<LinkRef>)`
  - `grammar::render_flow_bullet(&FlowBullet) -> String`, `grammar::render_flow_heading(&FlowNodeSyntax) -> String`
  - `grammar::parse_link_ref(&str) -> Option<LinkRef>` (whole-string `[T](./s.md)` match)

- [ ] **Step 1: Write the failing tests** — append to `grammar.rs` `tests`:

```rust
    use crate::model::FlowNodeKind;
    use crate::syntax::{FlowBullet, FlowTargetRef};

    #[test]
    fn parses_full_transition_bullet() {
        let FlowBullet::Transition(t) =
            parse_flow_bullet("- on `ship` when `paid` transitions to Shipped carries [Order](./order.md): `notify`").unwrap()
        else { panic!("expected a transition") };
        assert_eq!(t.trigger.as_deref(), Some("ship"));
        assert_eq!(t.guard.as_deref(), Some("paid"));
        assert!(!t.is_else);
        assert_eq!(t.target, FlowTargetRef::Local("Shipped".to_string()));
        assert_eq!(t.carries.as_ref().unwrap().slug, "order");
        assert_eq!(t.effect.as_deref(), Some("notify"));
    }

    #[test]
    fn parses_completion_else_and_link_target_transitions() {
        let FlowBullet::Transition(t) = parse_flow_bullet("- transitions to final").unwrap() else { panic!() };
        assert_eq!(t.target, FlowTargetRef::Local("final".to_string()));
        assert!(t.trigger.is_none() && t.guard.is_none() && !t.is_else);

        let FlowBullet::Transition(t) = parse_flow_bullet("- else transitions to Hold").unwrap() else { panic!() };
        assert!(t.is_else);

        let FlowBullet::Transition(t) =
            parse_flow_bullet("- transitions to [Fulfilment](./fulfilment.md)").unwrap() else { panic!() };
        assert!(matches!(t.target, FlowTargetRef::Link(ref l) if l.slug == "fulfilment"));
    }

    #[test]
    fn parses_internals_refines_and_partition() {
        assert_eq!(parse_flow_bullet("- entry: `reserveStock`").unwrap(), FlowBullet::Entry("reserveStock".to_string()));
        assert_eq!(parse_flow_bullet("- do: `poll`").unwrap(), FlowBullet::Do("poll".to_string()));
        assert_eq!(parse_flow_bullet("- exit: `release`").unwrap(), FlowBullet::Exit("release".to_string()));
        assert!(matches!(parse_flow_bullet("- refines [SubFlow](./sub.md)").unwrap(), FlowBullet::Refines(ref l) if l.slug == "sub"));
        assert_eq!(parse_flow_bullet("- partition: Warehouse").unwrap(), FlowBullet::Partition("Warehouse".to_string()));
        assert!(parse_flow_bullet("- goes to X").is_err());
        assert!(parse_flow_bullet("- when paid transitions to X").is_err(), "guards must be backticked");
    }

    #[test]
    fn parses_flow_headings() {
        assert_eq!(parse_flow_heading("Draft"), (FlowNodeKind::Plain, "Draft".to_string(), None));
        assert_eq!(parse_flow_heading("initial"), (FlowNodeKind::Initial, "initial".to_string(), None));
        assert_eq!(parse_flow_heading("decision Ready to ship?"), (FlowNodeKind::Decision, "Ready to ship?".to_string(), None));
        let (k, id, obj) = parse_flow_heading("object [Order](./order.md)");
        assert_eq!(k, FlowNodeKind::Object);
        assert_eq!(id, "Order");
        assert_eq!(obj.unwrap().slug, "order");
    }

    #[test]
    fn flow_bullets_and_headings_round_trip() {
        for line in [
            "- on `place` when `items > 0` transitions to Placed",
            "- transitions to Deliver carries [Order](./order.md)",
            "- else transitions to Hold",
            "- transitions to Shipped: `notify`",
            "- entry: `reserveStock`",
            "- refines [SubFlow](./sub.md)",
            "- partition: Warehouse",
        ] {
            let b = parse_flow_bullet(line).unwrap();
            assert_eq!(render_flow_bullet(&b), line);
        }
    }
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p waml grammar::tests`
Expected: FAIL — `parse_flow_bullet` not found (compile error).

- [ ] **Step 3: Implement**

`model.rs` — add near `BehaviorKind`:

```rust
/// A flow node's closed kind set (heading keyword). `Plain` = no keyword →
/// action (activity) or state (state machine).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "lowercase"))]
pub enum FlowNodeKind {
    Initial,
    Final,
    Decision,
    Merge,
    Fork,
    Join,
    Object,
    Plain,
}

impl FlowNodeKind {
    pub fn keyword(self) -> Option<&'static str> {
        match self {
            FlowNodeKind::Initial => Some("initial"),
            FlowNodeKind::Final => Some("final"),
            FlowNodeKind::Decision => Some("decision"),
            FlowNodeKind::Merge => Some("merge"),
            FlowNodeKind::Fork => Some("fork"),
            FlowNodeKind::Join => Some("join"),
            FlowNodeKind::Object => Some("object"),
            FlowNodeKind::Plain => None,
        }
    }
    pub fn from_keyword(s: &str) -> Option<FlowNodeKind> {
        match s {
            "initial" => Some(FlowNodeKind::Initial),
            "final" => Some(FlowNodeKind::Final),
            "decision" => Some(FlowNodeKind::Decision),
            "merge" => Some(FlowNodeKind::Merge),
            "fork" => Some(FlowNodeKind::Fork),
            "join" => Some(FlowNodeKind::Join),
            "object" => Some(FlowNodeKind::Object),
            _ => None,
        }
    }
}
```

`syntax.rs` — add after `MemberGroup`:

```rust
/// A parsed `[Title](./slug.md)` reference (unresolved slug stem).
#[derive(Debug, Clone, PartialEq)]
pub struct LinkRef {
    pub title: String,
    pub slug: String,
}

/// A flow edge's target: a bare local vertex label, or a cross-document link.
#[derive(Debug, Clone, PartialEq)]
pub enum FlowTargetRef {
    Local(String),
    Link(LinkRef),
}

/// One `transitions` bullet: `[on `t`] [when `g`|else] transitions to <target>
/// [carries <link>] [: `effect`]`.
#[derive(Debug, Clone, PartialEq)]
pub struct FlowTransition {
    pub trigger: Option<String>,
    pub guard: Option<String>,
    pub is_else: bool,
    pub target: FlowTargetRef,
    pub carries: Option<LinkRef>,
    pub effect: Option<String>,
    /// 1-based line within the document (0 until filled by the block parser).
    pub line: usize,
}

/// One bullet under a flow node heading.
#[derive(Debug, Clone, PartialEq)]
pub enum FlowBullet {
    Transition(FlowTransition),
    Entry(String),
    Do(String),
    Exit(String),
    Refines(LinkRef),
    Partition(String),
}

/// One `###` node in a `## Nodes` section. Identity = heading text minus the
/// leading kind keyword (the link title for `object` nodes).
#[derive(Debug, Clone, PartialEq)]
pub struct FlowNodeSyntax {
    pub kind: crate::model::FlowNodeKind,
    pub identity: String,
    pub object_ref: Option<LinkRef>,
    pub bullets: Vec<Line<FlowBullet>>,
    pub notes: Vec<Line<String>>,
    pub line: usize,
}

/// The `## Nodes` section of a flow document: one directed graph.
#[derive(Debug, Clone, PartialEq)]
pub struct FlowBlock {
    pub nodes: Vec<FlowNodeSyntax>,
    /// Non-heading content before the first `###` — preserved, never dropped.
    pub preamble_errors: Vec<ErrorNode>,
}
```

`grammar.rs` — add regexes near the others:

```rust
static FLOW_TRANSITION_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(concat!(
        r"^- ",
        r"(?:on `([^`]+)` )?",
        r"(?:when `([^`]+)` |(else) )?",
        r"transitions to (.+?)",
        r"(?: carries \[([^\]]+)\]\(\./(.+?)\.md\))?",
        r"(?::\s*`([^`]+)`)?$",
    ))
    .unwrap()
});
static FLOW_INTERNAL_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^- (entry|do|exit):\s*`([^`]+)`$").unwrap());
static FLOW_REFINES_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^- refines \[([^\]]+)\]\(\./(.+?)\.md\)$").unwrap());
static FLOW_PARTITION_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^- partition:\s*(\S.*)$").unwrap());
```

Add imports at the top of `grammar.rs`: extend the `crate::syntax` use to include `FlowBullet, FlowNodeSyntax, FlowTargetRef, FlowTransition, LinkRef` (and later Task 9's `FlowBlock`); add `use crate::model::FlowNodeKind;`.

Add the functions:

```rust
/// Whole-string `[Title](./slug.md)` reference, or None.
pub fn parse_link_ref(s: &str) -> Option<LinkRef> {
    LINK_RE
        .captures(s.trim())
        .map(|c| LinkRef { title: c[1].to_string(), slug: c[2].to_string() })
}

/// Human-readable message for a malformed flow bullet.
fn flow_error_message(line: &str) -> String {
    if line.contains("transitions") {
        "malformed transition — expected '[on `trigger`] [when `guard`|else] transitions to <target> [carries <link>] [: `effect`]' (expressions must be backticked)".to_string()
    } else {
        "unrecognized flow bullet — expected a transition, 'entry|do|exit: `effect`', 'refines <link>', or 'partition: <name>'".to_string()
    }
}

pub fn parse_flow_bullet(line: &str) -> Result<FlowBullet, LineError> {
    let trimmed = line.trim_end_matches('\r').trim();
    if let Some(m) = FLOW_TRANSITION_RE.captures(trimmed) {
        let raw_target = m[4].trim().to_string();
        let target = match parse_link_ref(&raw_target) {
            Some(l) => FlowTargetRef::Link(l),
            None => FlowTargetRef::Local(raw_target),
        };
        return Ok(FlowBullet::Transition(FlowTransition {
            trigger: m.get(1).map(|x| x.as_str().to_string()),
            guard: m.get(2).map(|x| x.as_str().to_string()),
            is_else: m.get(3).is_some(),
            target,
            carries: match (m.get(5), m.get(6)) {
                (Some(t), Some(s)) => Some(LinkRef { title: t.as_str().to_string(), slug: s.as_str().to_string() }),
                _ => None,
            },
            effect: m.get(7).map(|x| x.as_str().to_string()),
            line: 0,
        }));
    }
    if let Some(m) = FLOW_INTERNAL_RE.captures(trimmed) {
        let e = m[2].to_string();
        return Ok(match &m[1] {
            "entry" => FlowBullet::Entry(e),
            "do" => FlowBullet::Do(e),
            _ => FlowBullet::Exit(e),
        });
    }
    if let Some(m) = FLOW_REFINES_RE.captures(trimmed) {
        return Ok(FlowBullet::Refines(LinkRef { title: m[1].to_string(), slug: m[2].to_string() }));
    }
    if let Some(m) = FLOW_PARTITION_RE.captures(trimmed) {
        return Ok(FlowBullet::Partition(m[1].trim().to_string()));
    }
    Err(LineError { range: bullet_range(line), message: flow_error_message(trimmed) })
}

/// Split a `###` heading's text into (kind, identity, object link). The
/// identity is the text minus the leading kind keyword; a keyword-only heading
/// uses the keyword itself; an `object` node's identity is its link title.
pub fn parse_flow_heading(text: &str) -> (FlowNodeKind, String, Option<LinkRef>) {
    let t = text.trim();
    let (kw, rest) = match t.split_once(' ') {
        Some((a, b)) => (a, b.trim()),
        None => (t, ""),
    };
    match FlowNodeKind::from_keyword(kw) {
        None => (FlowNodeKind::Plain, t.to_string(), None),
        Some(k) if rest.is_empty() => (k, kw.to_string(), None),
        Some(FlowNodeKind::Object) => match parse_link_ref(rest) {
            Some(l) => (FlowNodeKind::Object, l.title.clone(), Some(l)),
            None => (FlowNodeKind::Object, rest.to_string(), None),
        },
        Some(k) => (k, rest.to_string(), None),
    }
}

pub fn render_flow_heading(n: &FlowNodeSyntax) -> String {
    match n.kind {
        FlowNodeKind::Plain => format!("### {}", n.identity),
        FlowNodeKind::Object => match &n.object_ref {
            Some(l) => format!("### object [{}](./{}.md)", l.title, l.slug),
            None => format!("### object {}", n.identity),
        },
        k => {
            let kw = k.keyword().expect("non-plain kinds have a keyword");
            if n.identity == kw { format!("### {kw}") } else { format!("### {kw} {}", n.identity) }
        }
    }
}

pub fn render_flow_bullet(b: &FlowBullet) -> String {
    match b {
        FlowBullet::Transition(t) => {
            let mut s = String::from("- ");
            if let Some(x) = &t.trigger {
                s.push_str(&format!("on `{x}` "));
            }
            if let Some(g) = &t.guard {
                s.push_str(&format!("when `{g}` "));
            } else if t.is_else {
                s.push_str("else ");
            }
            s.push_str("transitions to ");
            match &t.target {
                FlowTargetRef::Local(n) => s.push_str(n),
                FlowTargetRef::Link(l) => s.push_str(&format!("[{}](./{}.md)", l.title, l.slug)),
            }
            if let Some(c) = &t.carries {
                s.push_str(&format!(" carries [{}](./{}.md)", c.title, c.slug));
            }
            if let Some(e) = &t.effect {
                s.push_str(&format!(": `{e}`"));
            }
            s
        }
        FlowBullet::Entry(e) => format!("- entry: `{e}`"),
        FlowBullet::Do(e) => format!("- do: `{e}`"),
        FlowBullet::Exit(e) => format!("- exit: `{e}`"),
        FlowBullet::Refines(l) => format!("- refines [{}](./{}.md)", l.title, l.slug),
        FlowBullet::Partition(p) => format!("- partition: {p}"),
    }
}
```

- [ ] **Step 4: Run to verify pass**

Run: `cargo test -p waml`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/waml/src/model.rs crates/waml/src/syntax.rs crates/waml/src/grammar.rs
git commit -m "feat(grammar): flow node/transition line grammar with round-trip renderers"
```

---

### Task 9: `## Nodes` block parser, `Section::Nodes` wiring, lossless serialize

**Files:**
- Modify: `crates/waml/src/diagnostic.rs` (add `MalformedFlowBullet`)
- Modify: `crates/waml/src/grammar.rs` (`parse_flow_block`, `render_flow_block`)
- Modify: `crates/waml/src/syntax.rs` (`Section::Nodes(FlowBlock)`)
- Modify: `crates/waml/src/parse.rs` (`walk_section`, `diagnostics_of`)
- Modify: `crates/waml/src/serialize.rs` (`section_order`, `render_section`)
- Test: inline in `parse.rs` and `serialize.rs`

**Interfaces:**
- Consumes: Task 8's flow syntax + line grammar.
- Produces: `grammar::parse_flow_block(content: &str, content_abs_start: usize, src: &str) -> FlowBlock`; `grammar::render_flow_block(&FlowBlock) -> String` (includes the `## Nodes` heading); `Section::Nodes(FlowBlock)`; `DiagCode::MalformedFlowBullet` (`"malformed-flow-bullet"`, Error). Task 10 reads `Section::Nodes` from parsed documents.

- [ ] **Step 1: Write the failing tests**

In `parse.rs` `tests`:

```rust
    const LIFECYCLE: &str = "---\ntype: uml.StateMachine\ntitle: Order Lifecycle\ndescribes: [Order](./order.md)\n---\n# Order Lifecycle\n\n## Nodes\n\n### initial\n- transitions to Draft\n\n### Draft\n- on `place` when `items > 0` transitions to Placed\n- on `cancel` transitions to Cancelled\n\n#### Notes\n- Auto-expires after 24h.\n\n### Placed\n- entry: `reserveStock`\n- on `ship` transitions to Shipped: `notify`\n\n### Shipped\n- on `deliver` transitions to final\n\n### Cancelled\n- transitions to final\n\n### final\n";

    #[test]
    fn parses_flow_nodes_section() {
        use crate::model::FlowNodeKind;
        use crate::syntax::{FlowBullet, Section};
        let doc = parse_document(LIFECYCLE);
        let block = doc.sections.iter().find_map(|s| match s {
            Section::Nodes(b) => Some(b),
            _ => None,
        }).expect("## Nodes must parse into Section::Nodes");
        assert_eq!(block.nodes.len(), 7);
        assert_eq!(block.nodes[0].kind, FlowNodeKind::Initial);
        assert_eq!(block.nodes[1].identity, "Draft");
        assert_eq!(block.nodes[1].bullets.len(), 2);
        assert_eq!(block.nodes[1].notes.iter().filter_map(crate::syntax::Line::parsed).next().unwrap(), "Auto-expires after 24h.");
        assert!(matches!(block.nodes[2].bullets[0].parsed().unwrap(), FlowBullet::Entry(e) if e == "reserveStock"));
        assert_eq!(block.nodes[6].kind, FlowNodeKind::Final);
    }

    #[test]
    fn malformed_flow_bullet_is_preserved_and_diagnosed() {
        let src = "---\ntype: uml.Activity\ntitle: A\n---\n# A\n\n## Nodes\n\n### Ship\n- goes to Deliver\n";
        let (doc, diags) = parse(src);
        let d = diags.iter().find(|d| d.code == DiagCode::MalformedFlowBullet).unwrap();
        assert_eq!(d.line, 10);
        // preserved, not dropped
        use crate::syntax::{Line, Section};
        let block = doc.sections.iter().find_map(|s| match s { Section::Nodes(b) => Some(b), _ => None }).unwrap();
        assert!(matches!(&block.nodes[0].bullets[0], Line::Error(e) if e.raw.contains("goes to Deliver")));
    }
```

In `serialize.rs` `tests`:

```rust
    #[test]
    fn flow_document_serialize_is_a_semantic_fixpoint() {
        let src = "---\ntype: uml.StateMachine\ntitle: Order Lifecycle\ndescribes: [Order](./order.md)\n---\n# Order Lifecycle\n\n## Nodes\n\n### initial\n- transitions to Draft\n\n### Draft\n- on `place` when `items > 0` transitions to Placed\n\n#### Notes\n- Auto-expires after 24h.\n\n### decision Ready to ship?\n- when `paid and inStock` transitions to Ship\n- else transitions to Hold\n\n### object [Order](./order.md)\n\n### Ship\n- transitions to Deliver carries [Order](./order.md)\n\n### final\n";
        let once = serialize_document(&parse_document(src));
        let twice = serialize_document(&parse_document(&once));
        assert_eq!(once, twice);
        assert!(once.contains("### decision Ready to ship?"));
        assert!(once.contains("- else transitions to Hold"));
        assert!(once.contains("### object [Order](./order.md)"));
        assert!(once.contains("#### Notes\n- Auto-expires after 24h."));
    }
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p waml`
Expected: FAIL — `Section::Nodes` / `MalformedFlowBullet` unknown (compile errors).

- [ ] **Step 3: Implement**

`diagnostic.rs` — add `MalformedFlowBullet` to `DiagCode` (after `MalformedRelationship`), with `as_str` arm `"malformed-flow-bullet"`. Default severity stays Error (no change to `severity()` needed).

`syntax.rs` — add to `enum Section`:

```rust
    /// A flow document's `## Nodes` section (one directed graph).
    Nodes(FlowBlock),
```

`grammar.rs` — add block parser + renderer:

```rust
/// Parse the raw text under `## Nodes` into a flow graph block. Each `###`
/// heading opens a node; `#### Notes` opens the current node's notes; bullets
/// parse via `parse_flow_bullet`. Malformed or stray lines are preserved as
/// positioned `Line::Error`s (never dropped).
pub fn parse_flow_block(content: &str, content_abs_start: usize, src: &str) -> FlowBlock {
    let mut nodes: Vec<FlowNodeSyntax> = Vec::new();
    let mut preamble_errors: Vec<ErrorNode> = Vec::new();
    let mut in_notes = false;
    let mut fence: Option<char> = None;
    let mut offset = 0usize;

    for raw in content.split('\n') {
        let line_start = offset;
        offset += raw.len() + 1;
        let line = raw.trim_end_matches('\r');
        let t = line.trim();

        if let Some(marker) = fence {
            let delim = if marker == '`' { "```" } else { "~~~" };
            if t.starts_with(delim) {
                fence = None;
            }
            continue;
        }
        if t.starts_with("```") {
            fence = Some('`');
            continue;
        }
        if t.starts_with("~~~") {
            fence = Some('~');
            continue;
        }
        if t.is_empty() {
            continue;
        }

        let line_no = crate::parse::line_at(src, content_abs_start + line_start);

        if let Some(rest) = t.strip_prefix("### ") {
            let (kind, identity, object_ref) = parse_flow_heading(rest);
            nodes.push(FlowNodeSyntax { kind, identity, object_ref, bullets: vec![], notes: vec![], line: line_no });
            in_notes = false;
            continue;
        }
        if let Some(rest) = t.strip_prefix("#### ") {
            if rest.trim().eq_ignore_ascii_case("notes") && !nodes.is_empty() {
                in_notes = true;
                continue;
            }
            // Unrecognized sub-heading → preserved droppable line.
        }

        let droppable = || ErrorNode {
            raw: raw.to_string(),
            line: line_no,
            span: bullet_range(raw),
            code: DiagCode::DroppableContent,
            message: crate::parse::DROPPABLE_MSG.to_string(),
        };
        let Some(node) = nodes.last_mut() else {
            preamble_errors.push(droppable());
            continue;
        };
        if in_notes {
            match parse_value_line(raw) {
                Ok(v) => node.notes.push(Line::Parsed(v)),
                Err(_) => node.notes.push(Line::Error(droppable())),
            }
        } else if t.starts_with("- ") {
            match parse_flow_bullet(raw) {
                Ok(mut b) => {
                    if let FlowBullet::Transition(ref mut tr) = b {
                        tr.line = line_no;
                    }
                    node.bullets.push(Line::Parsed(b));
                }
                Err(e) => node.bullets.push(Line::Error(ErrorNode {
                    raw: raw.to_string(),
                    line: line_no,
                    span: e.range,
                    code: DiagCode::MalformedFlowBullet,
                    message: e.message,
                })),
            }
        } else {
            node.bullets.push(Line::Error(droppable()));
        }
    }
    FlowBlock { nodes, preamble_errors }
}

/// Render a flow block, `## Nodes` heading included, as canonical Markdown.
pub fn render_flow_block(block: &FlowBlock) -> String {
    let mut out = String::from("## Nodes");
    for e in &block.preamble_errors {
        out.push('\n');
        out.push_str(&e.raw);
    }
    for n in &block.nodes {
        out.push_str("\n\n");
        out.push_str(&render_flow_heading(n));
        for b in &n.bullets {
            out.push('\n');
            match b {
                Line::Parsed(x) => out.push_str(&render_flow_bullet(x)),
                Line::Error(e) => out.push_str(&e.raw),
            }
        }
        if !n.notes.is_empty() {
            out.push_str("\n\n#### Notes");
            for m in &n.notes {
                out.push('\n');
                match m {
                    Line::Parsed(v) => out.push_str(&format!("- {v}")),
                    Line::Error(e) => out.push_str(&e.raw),
                }
            }
        }
    }
    out
}
```

(Also extend `grammar.rs`'s `use crate::syntax::{…}` list with `FlowBlock`.)

`parse.rs` — in `walk_section`, add before the `_ =>` arm:

```rust
        "nodes" => Section::Nodes(crate::grammar::parse_flow_block(content, content_abs_start, src)),
```

In `diagnostics_of`, add an arm:

```rust
            Section::Nodes(block) => {
                for e in &block.preamble_errors {
                    out.push(Diagnostic::new(e.code, e.message.clone(), "", e.line).with_span(e.span));
                }
                for n in &block.nodes {
                    push_line_errors(&n.bullets, &mut out);
                    push_line_errors(&n.notes, &mut out);
                }
            }
```

`serialize.rs` — replace `section_order` (Nodes slots between Notes and Members):

```rust
fn section_order(s: &Section) -> u8 {
    match s {
        Section::Body(_) => 0,
        Section::Attributes(_) => 1,
        Section::Values(_) => 2,
        Section::Relationships(_) => 3,
        Section::Notes(_) => 4,
        Section::Nodes(_) => 5,
        Section::Members(_) => 6,
        Section::Layout(_) => 7,
        Section::Unknown { .. } => 8,
    }
}
```

In `render_section` add:

```rust
        Section::Nodes(block) => crate::grammar::render_flow_block(block),
```

- [ ] **Step 4: Run to verify pass**

Run: `cargo test -p waml`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/waml/src/diagnostic.rs crates/waml/src/grammar.rs crates/waml/src/syntax.rs crates/waml/src/parse.rs crates/waml/src/serialize.rs
git commit -m "feat(parse): lossless ## Nodes flow-graph section with heading nodes and notes"
```

---

### Task 10: `FlowDoc` model + `build_flows` resolution + wire shape

**Files:**
- Modify: `crates/waml/src/model.rs` (FlowFlavor, FlowNode, FlowEdge, FlowDoc, `Model.flows`)
- Modify: `crates/waml/src/parse.rs` (`build_flows`, `build_model`)
- Test: inline in `parse.rs`; wire pin in `crates/waml/tests/serde_shape.rs`

**Interfaces:**
- Consumes: `Section::Nodes(FlowBlock)` (Task 9), `ClassifierType::Behavior` (Task 7), `okf::resolve_href` (existing).
- Produces (wire/TS names in serde renames — Task 12 mirrors them exactly):

```rust
FlowFlavor: Activity | StateMachine            // wire: "activity" | "stateMachine"
FlowNode { id, kind, object_ref→"objectRef", partition, entry, do_→"do", exit, refines, notes }
FlowEdge { from, to, to_ref→"toRef", trigger, guard, is_else→"else", effect, carries }
FlowDoc { key, title, flavor, describes, nodes, edges }
Model.flows: Vec<FlowDoc>                       // wire: "flows", omitted when empty
```

- [ ] **Step 1: Write the failing tests**

In `parse.rs` `model_tests`:

```rust
    #[test]
    fn builds_flow_doc_with_resolved_links_and_edges() {
        use crate::model::{FlowFlavor, FlowNodeKind};
        let b = vec![
            ("m/order.md".into(), "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n".into()),
            ("m/sub.md".into(), "---\ntype: uml.Activity\ntitle: Sub\n---\n# Sub\n\n## Nodes\n\n### initial\n- transitions to final\n\n### final\n".into()),
            ("m/lifecycle.md".into(),
             "---\ntype: uml.StateMachine\ntitle: Order Lifecycle\ndescribes: [Order](./order.md)\n---\n# Order Lifecycle\n\n## Nodes\n\n### initial\n- transitions to Draft\n\n### Draft\n- on `place` when `items > 0` transitions to Placed: `reserve`\n- partition: Sales\n\n### Placed\n- entry: `reserveStock`\n- refines [Sub](./sub.md)\n- transitions to Ship carries [Order](./order.md)\n\n### Ship\n- transitions to final\n\n### final\n".into()),
        ];
        let m = build_model(&b);
        assert_eq!(m.flows.len(), 2);
        let f = m.flows.iter().find(|f| f.key == "m/lifecycle").unwrap();
        assert_eq!(f.flavor, FlowFlavor::StateMachine);
        assert_eq!(f.describes.as_deref(), Some("m/order"));
        assert_eq!(f.nodes.len(), 5);
        assert_eq!(f.nodes[0].kind, FlowNodeKind::Initial);
        assert_eq!(f.nodes[1].partition.as_deref(), Some("Sales"));
        assert_eq!(f.nodes[2].entry.as_deref(), Some("reserveStock"));
        assert_eq!(f.nodes[2].refines.as_deref(), Some("m/sub"));
        assert_eq!(f.edges.len(), 4);
        let placed = f.edges.iter().find(|e| e.to == "Placed").unwrap();
        assert_eq!(placed.trigger.as_deref(), Some("place"));
        assert_eq!(placed.guard.as_deref(), Some("items > 0"));
        assert_eq!(placed.effect.as_deref(), Some("reserve"));
        let ship = f.edges.iter().find(|e| e.to == "Ship").unwrap();
        assert_eq!(ship.carries.as_deref(), Some("m/order"));
    }
```

In `crates/waml/tests/serde_shape.rs`:

```rust
#[test]
fn flow_doc_json_matches_ts_field_names() {
    let b = vec![
        ("m/order.md".to_string(), "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n".to_string()),
        ("m/lifecycle.md".to_string(),
         "---\ntype: uml.StateMachine\ntitle: Order Lifecycle\ndescribes: [Order](./order.md)\n---\n# Order Lifecycle\n\n## Nodes\n\n### initial\n- transitions to Draft\n\n### Draft\n- on `place` when `items > 0` transitions to Placed: `reserve`\n- else transitions to Cancelled\n\n### Placed\n- entry: `reserveStock`\n\n### Cancelled\n\n### final\n".to_string()),
    ];
    let m = build_model(&b);
    let v = serde_json::to_value(&m).unwrap();
    let f = &v["flows"][0];
    assert_eq!(f["key"], "m/lifecycle");
    assert_eq!(f["flavor"], "stateMachine");
    assert_eq!(f["describes"], "m/order");
    assert_eq!(f["nodes"][0]["kind"], "initial");
    assert_eq!(f["nodes"][2]["entry"], "reserveStock");
    let e = &f["edges"][1];
    assert_eq!(e["from"], "Draft");
    assert_eq!(e["trigger"], "place");
    assert_eq!(e["guard"], "items > 0");
    assert_eq!(e["effect"], "reserve");
    assert_eq!(f["edges"][2]["else"], true);
    // classifier-only models omit the field entirely
    let m2 = build_model(&vec![("a.md".to_string(), "---\ntype: uml.Class\ntitle: A\n---\n# A\n".to_string())]);
    let v2 = serde_json::to_value(&m2).unwrap();
    assert!(v2.get("flows").is_none());
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p waml`
Expected: FAIL — `Model` has no field `flows` (compile error).

- [ ] **Step 3: Implement**

`model.rs` — add after `FlowNodeKind`:

```rust
/// Flow flavor: tunes rendering only — one grammar for both.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub enum FlowFlavor {
    Activity,
    StateMachine,
}

/// A resolved node of a flow document.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct FlowNode {
    /// Heading text minus the kind keyword — the name transitions resolve against.
    pub id: String,
    pub kind: FlowNodeKind,
    /// Resolved key of an `object` node's typing classifier.
    #[cfg_attr(feature = "serde", serde(rename = "objectRef", default, skip_serializing_if = "Option::is_none"))]
    pub object_ref: Option<String>,
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Option::is_none"))]
    pub partition: Option<String>,
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Option::is_none"))]
    pub entry: Option<String>,
    #[cfg_attr(feature = "serde", serde(rename = "do", default, skip_serializing_if = "Option::is_none"))]
    pub do_: Option<String>,
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Option::is_none"))]
    pub exit: Option<String>,
    /// Resolved key of the flow document this composite/call-behavior refines.
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Option::is_none"))]
    pub refines: Option<String>,
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Vec::is_empty"))]
    pub notes: Vec<String>,
}

/// A resolved transition (flow edge). Source/target are node identities.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct FlowEdge {
    pub from: String,
    /// Local node identity, or the link title for a cross-document target.
    pub to: String,
    /// Resolved key when the target was a cross-document link.
    #[cfg_attr(feature = "serde", serde(rename = "toRef", default, skip_serializing_if = "Option::is_none"))]
    pub to_ref: Option<String>,
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Option::is_none"))]
    pub trigger: Option<String>,
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Option::is_none"))]
    pub guard: Option<String>,
    /// Decision default branch (`else transitions to …`).
    #[cfg_attr(feature = "serde", serde(rename = "else", default, skip_serializing_if = "is_false"))]
    pub is_else: bool,
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Option::is_none"))]
    pub effect: Option<String>,
    /// Resolved key of the carried object type (`carries <link>` object flow).
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Option::is_none"))]
    pub carries: Option<String>,
}

/// One flow document: one self-rendering directed graph (model AND view).
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct FlowDoc {
    pub key: String,
    pub title: String,
    pub flavor: FlowFlavor,
    /// Resolved key of the entity this behavior describes (frontmatter link).
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Option::is_none"))]
    pub describes: Option<String>,
    pub nodes: Vec<FlowNode>,
    pub edges: Vec<FlowEdge>,
}
```

Add to `struct Model`:

```rust
    /// Flow-substrate behavior documents (uml.Activity / uml.StateMachine).
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Vec::is_empty"))]
    pub flows: Vec<FlowDoc>,
```

`parse.rs` — add (near `build_diagrams`); extend the `crate::model` import list at the top of the file with `BehaviorKind, FlowDoc, FlowEdge, FlowFlavor, FlowNode`:

```rust
/// Resolve a frontmatter `describes: [T](./t.md)` link against the classifier keyset.
fn resolve_describes(p: &ParsedDoc, keyset: &HashSet<&str>) -> Option<String> {
    p.doc
        .frontmatter
        .get_str("describes")
        .and_then(crate::grammar::parse_link_ref)
        .map(|l| crate::okf::resolve_href(&p.path, &l.slug))
        .filter(|k| keyset.contains(k.as_str()))
}

fn build_flows(parsed: &[ParsedDoc], keyset: &HashSet<&str>) -> Vec<FlowDoc> {
    use crate::syntax::{FlowBullet, FlowTargetRef};
    let flow_keys: HashSet<String> = parsed
        .iter()
        .filter(|p| matches!(p.ty, ClassifierType::Behavior(BehaviorKind::Activity | BehaviorKind::StateMachine)))
        .map(|p| p.id.clone())
        .collect();
    let mut out = Vec::new();
    for p in parsed {
        let flavor = match p.ty {
            ClassifierType::Behavior(BehaviorKind::Activity) => FlowFlavor::Activity,
            ClassifierType::Behavior(BehaviorKind::StateMachine) => FlowFlavor::StateMachine,
            _ => continue,
        };
        let mut nodes = Vec::new();
        let mut edges = Vec::new();
        for s in &p.doc.sections {
            let Section::Nodes(block) = s else { continue };
            for n in &block.nodes {
                let mut fnode = FlowNode {
                    id: n.identity.clone(),
                    kind: n.kind,
                    object_ref: n
                        .object_ref
                        .as_ref()
                        .map(|l| crate::okf::resolve_href(&p.path, &l.slug))
                        .filter(|k| keyset.contains(k.as_str())),
                    partition: None,
                    entry: None,
                    do_: None,
                    exit: None,
                    refines: None,
                    notes: n.notes.iter().filter_map(Line::parsed).cloned().collect(),
                };
                for b in n.bullets.iter().filter_map(Line::parsed) {
                    match b {
                        FlowBullet::Transition(t) => {
                            let (to, to_ref) = match &t.target {
                                FlowTargetRef::Local(name) => (name.clone(), None),
                                FlowTargetRef::Link(l) => {
                                    let r = crate::okf::resolve_href(&p.path, &l.slug);
                                    (l.title.clone(), flow_keys.contains(&r).then_some(r))
                                }
                            };
                            edges.push(FlowEdge {
                                from: n.identity.clone(),
                                to,
                                to_ref,
                                trigger: t.trigger.clone(),
                                guard: t.guard.clone(),
                                is_else: t.is_else,
                                effect: t.effect.clone(),
                                carries: t
                                    .carries
                                    .as_ref()
                                    .map(|l| crate::okf::resolve_href(&p.path, &l.slug))
                                    .filter(|k| keyset.contains(k.as_str())),
                            });
                        }
                        FlowBullet::Entry(e) => fnode.entry = Some(e.clone()),
                        FlowBullet::Do(e) => fnode.do_ = Some(e.clone()),
                        FlowBullet::Exit(e) => fnode.exit = Some(e.clone()),
                        FlowBullet::Refines(l) => {
                            let r = crate::okf::resolve_href(&p.path, &l.slug);
                            fnode.refines = flow_keys.contains(&r).then_some(r);
                        }
                        FlowBullet::Partition(name) => fnode.partition = Some(name.clone()),
                    }
                }
                nodes.push(fnode);
            }
        }
        out.push(FlowDoc {
            key: p.id.clone(),
            title: doc_title(p),
            flavor,
            describes: resolve_describes(p, keyset),
            nodes,
            edges,
        });
    }
    out
}
```

In `build_model`, add before the final `Model` literal and extend it:

```rust
    let flows = build_flows(&parsed, &keyset);

    Model { nodes, edges, diagrams, path, packages, flows, ..Default::default() }
```

(`..Default::default()` also covers Task 18's `interactions` when it lands; until then it is a no-op.)

- [ ] **Step 4: Run to verify pass**

Run: `cargo test -p waml`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/waml/src/model.rs crates/waml/src/parse.rs crates/waml/tests/serde_shape.rs
git commit -m "feat(model): FlowDoc resolution — describes/refines/carries links, flow edges on the wire"
```

---

### Task 11: Validate flow documents — duplicate identities, unresolved targets

**Files:**
- Modify: `crates/waml/src/diagnostic.rs` (add `DuplicateFlowNode`)
- Modify: `crates/waml/src/validate.rs`
- Test: inline in `validate.rs`

**Interfaces:**
- Consumes: `Section::Nodes` (Task 9), `FlowTransition.line` (Task 8).
- Produces: `DiagCode::DuplicateFlowNode` (`"duplicate-flow-node"`, Error); `UnresolvedTarget` warnings for bare transition targets matching no local `###` node. LSP and CLI pick these up automatically via `validate()`.

- [ ] **Step 1: Write the failing tests** — append to `validate.rs` `tests`:

```rust
    #[test]
    fn flow_doc_with_clean_graph_validates_clean() {
        let b = vec![("f/a.md".into(),
            "---\ntype: uml.Activity\ntitle: A\n---\n# A\n\n## Nodes\n\n### initial\n- transitions to Work\n\n### Work\n- transitions to final\n\n### final\n".into())];
        let d = validate(&b);
        assert!(d.is_empty(), "got: {d:?}");
    }

    #[test]
    fn flags_unresolved_local_transition_target() {
        let b = vec![("f/a.md".into(),
            "---\ntype: uml.Activity\ntitle: A\n---\n# A\n\n## Nodes\n\n### initial\n- transitions to Ghost\n\n### final\n".into())];
        let d = validate(&b);
        let t = d.iter().find(|x| x.code == DiagCode::UnresolvedTarget).unwrap();
        assert_eq!(t.severity, Severity::Warning);
        assert_eq!(t.line, 10);
    }

    #[test]
    fn flags_duplicate_flow_node_identity() {
        let b = vec![("f/a.md".into(),
            "---\ntype: uml.StateMachine\ntitle: A\n---\n# A\n\n## Nodes\n\n### Draft\n\n### Draft\n".into())];
        let d = validate(&b);
        assert!(d.iter().any(|x| x.code == DiagCode::DuplicateFlowNode));
    }
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p waml validate::tests`
Expected: FAIL — `DuplicateFlowNode` unknown (compile error).

- [ ] **Step 3: Implement**

`diagnostic.rs` — add `DuplicateFlowNode` variant with `as_str` arm `"duplicate-flow-node"` (Error severity by default).

`validate.rs` — in `link`'s per-document section loop, add an arm to the `match s`:

```rust
                Section::Nodes(block) => {
                    use crate::syntax::{FlowBullet, FlowTargetRef};
                    let mut counts: HashMap<&str, usize> = HashMap::new();
                    for n in &block.nodes {
                        *counts.entry(n.identity.as_str()).or_insert(0) += 1;
                    }
                    for n in &block.nodes {
                        if counts[n.identity.as_str()] > 1 {
                            diags.push(Diagnostic::new(
                                DiagCode::DuplicateFlowNode,
                                format!("duplicate node identity '{}' — transition targets resolve by identity", n.identity),
                                path,
                                n.line,
                            ));
                        }
                        for b in n.bullets.iter().filter_map(Line::parsed) {
                            let FlowBullet::Transition(t) = b else { continue };
                            if let FlowTargetRef::Local(name) = &t.target {
                                if !counts.contains_key(name.as_str()) {
                                    diags.push(Diagnostic::warn(
                                        DiagCode::UnresolvedTarget,
                                        format!("transition target '{name}' matches no '###' node in this document"),
                                        path,
                                        t.line,
                                    ));
                                }
                            }
                        }
                    }
                }
```

Also update the keyset builder at the top of `link` so behavior docs are not relationship targets (they are not classifiers):

```rust
        if *ty != ClassifierType::Diagram && !matches!(ty, ClassifierType::Behavior(_)) {
            keyset.insert(slug);
        }
```

Add `Section` import coverage (the file already imports `Section`), and `use crate::syntax::Line;` is already present.

- [ ] **Step 4: Run to verify pass**

Run: `cargo test -p waml`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/waml/src/diagnostic.rs crates/waml/src/validate.rs
git commit -m "feat(validate): flow-graph checks — duplicate identities, unresolved transition targets"
```

---

### Task 12: TS wire types + `toModelGraph` pass-through

**Files:**
- Modify: `packages/okf/src/types.ts` (FlowDoc family, `ModelGraph.flows`)
- Modify: `packages/core/src/state/overlay.ts` (`RustModel.flows`, `toModelGraph`)
- Test: `packages/core/src/state/overlay.test.ts`

**Interfaces:**
- Consumes: Task 10's wire JSON (`flavor: "activity"|"stateMachine"`, `objectRef`, `do`, `toRef`, `else`).
- Produces: TS `FlowFlavor`, `FlowNodeKind`, `FlowNode`, `FlowEdge`, `FlowDoc` types exported from `@waml/okf`; `ModelGraph.flows?: FlowDoc[]`; `toModelGraph` copies `model.flows ?? []`. Tasks 13–14 import these.

- [ ] **Step 1: Write the failing test** — append to `packages/core/src/state/overlay.test.ts`:

```ts
import type { FlowDoc } from "@waml/okf";

it("passes flow docs through to the ModelGraph", () => {
  const flow: FlowDoc = {
    key: "m/lifecycle",
    title: "Order Lifecycle",
    flavor: "stateMachine",
    describes: "m/order",
    nodes: [{ id: "initial", kind: "initial" }, { id: "Draft", kind: "plain", entry: "reserveStock" }],
    edges: [{ from: "initial", to: "Draft" }],
  };
  const rust = { nodes: [], edges: [], diagrams: [], path: "", packages: [], flows: [flow] };
  const g = toModelGraph(rust as never, emptyOverlay());
  expect(g.flows).toEqual([flow]);
});
```

(Reuse the file's existing imports of `toModelGraph`/`emptyOverlay`; add only the type import.)

- [ ] **Step 2: Run to verify failure**

Run: `pnpm --filter @waml/core test -- overlay`
Expected: FAIL — `FlowDoc` not exported / `flows` missing on `ModelGraph`.

- [ ] **Step 3: Implement**

`packages/okf/src/types.ts` — add after the `Diagram` interface:

```ts
// ── Flow substrate (uml.Activity / uml.StateMachine) ─────────────────────────
// One document = one directed graph; self-rendering (layout at render time,
// no stored coordinates). Mirrors Rust model.rs FlowDoc/FlowNode/FlowEdge.

export type FlowFlavor = "activity" | "stateMachine";

export type FlowNodeKind = "initial" | "final" | "decision" | "merge" | "fork" | "join" | "object" | "plain";

export interface FlowNode {
  /** Heading text minus the kind keyword — the name transitions resolve against. */
  id: string;
  kind: FlowNodeKind;
  /** Resolved key of an `object` node's typing classifier. */
  objectRef?: string;
  partition?: string;
  entry?: string;
  do?: string;
  exit?: string;
  /** Resolved key of the flow doc this composite/call-behavior refines. */
  refines?: string;
  notes?: string[];
}

export interface FlowEdge {
  from: string;
  /** Local node identity, or the link title for a cross-document target. */
  to: string;
  /** Resolved key when the target was a cross-document link. */
  toRef?: string;
  trigger?: string;
  guard?: string;
  /** Decision default branch (`else transitions to …`). */
  else?: boolean;
  effect?: string;
  /** Resolved key of the carried object type (object flow). */
  carries?: string;
}

export interface FlowDoc {
  key: string;
  title: string;
  flavor: FlowFlavor;
  /** Resolved key of the entity this behavior describes. */
  describes?: string;
  nodes: FlowNode[];
  edges: FlowEdge[];
}
```

Add to `ModelGraph`:

```ts
  /** Flow-substrate behavior documents (self-rendering; absent on legacy graphs). */
  flows?: FlowDoc[];
```

`packages/core/src/state/overlay.ts` — add `FlowDoc` to the `@waml/okf` type import list; add to `RustModel`:

```ts
  /** Flow-substrate behavior docs; omitted by serde when empty. */
  flows?: FlowDoc[];
```

In `toModelGraph`'s return statement:

```ts
  return { nodes, edges, diagrams, path: model.path ?? "", packages, flows: model.flows ?? [] };
```

- [ ] **Step 4: Run to verify pass**

Run: `pnpm build:wasm && pnpm --filter @waml/core test && pnpm --filter @waml/web test`
Expected: PASS (wasm rebuild bundles the Rust wire changes from Tasks 7–11).

- [ ] **Step 5: Commit**

```bash
git add packages/okf/src/types.ts packages/core/src/state/overlay.ts packages/core/src/state/overlay.test.ts
git commit -m "feat(ts): FlowDoc wire types and ModelGraph pass-through"
```

---

### Task 13: Web flow graph builder — dagre layout + transition labels

**Files:**
- Create: `packages/web/src/canvas/flowGraph.ts`
- Test: `packages/web/src/canvas/flowGraph.test.ts`

**Interfaces:**
- Consumes: `FlowDoc`, `FlowEdge`, `FlowNode` from `@waml/okf` (Task 12); `dagre` (already a web dependency).
- Produces: `flowToRf(doc: FlowDoc): { nodes: Node[]; edges: Edge[] }` (node `type` ∈ `"flowStep" | "flowControl" | "flowObject"`, node `data = { node: FlowNode, flavor: FlowFlavor }`; edge `type: "transition"`, edge `data = { label: string, carries?: string }`); `transitionLabel(e: FlowEdge): string`; `flowNodeSize(n: FlowNode): { width: number; height: number }`. Task 14's components read exactly these `data` shapes.

- [ ] **Step 1: Write the failing test** — `packages/web/src/canvas/flowGraph.test.ts`:

```ts
import { describe, expect, it } from "vitest";
import type { FlowDoc } from "@waml/okf";
import { flowToRf, transitionLabel } from "./flowGraph";

const DOC: FlowDoc = {
  key: "m/lifecycle",
  title: "Order Lifecycle",
  flavor: "stateMachine",
  nodes: [
    { id: "initial", kind: "initial" },
    { id: "Draft", kind: "plain" },
    { id: "Ready to ship?", kind: "decision" },
    { id: "final", kind: "final" },
  ],
  edges: [
    { from: "initial", to: "Draft" },
    { from: "Draft", to: "Ready to ship?", trigger: "place", guard: "items > 0", effect: "reserve" },
    { from: "Ready to ship?", to: "final", else: true },
    { from: "Draft", to: "Missing" }, // unresolved target: not drawn, never errors
  ],
};

describe("transitionLabel", () => {
  it("renders UML 'trigger [guard] / effect' labels", () => {
    expect(transitionLabel(DOC.edges[1])).toBe("place [items > 0] / reserve");
    expect(transitionLabel(DOC.edges[2])).toBe("[else]");
    expect(transitionLabel(DOC.edges[0])).toBe("");
  });
});

describe("flowToRf", () => {
  it("lays out every node and maps kinds to component types", () => {
    const { nodes, edges } = flowToRf(DOC);
    expect(nodes).toHaveLength(4);
    expect(nodes.map((n) => n.type)).toEqual(["flowControl", "flowStep", "flowControl", "flowControl"]);
    // dagre TB: the initial node sits above the final node
    const y = (id: string) => nodes.find((n) => n.id === id)!.position.y;
    expect(y("initial")).toBeLessThan(y("final"));
    // the edge to a missing node is dropped, the rest are transitions
    expect(edges).toHaveLength(3);
    expect(edges.every((e) => e.type === "transition")).toBe(true);
  });
});
```

- [ ] **Step 2: Run to verify failure**

Run: `pnpm --filter @waml/web test -- flowGraph`
Expected: FAIL — module not found.

- [ ] **Step 3: Implement** — `packages/web/src/canvas/flowGraph.ts`:

```ts
import dagre from "@dagrejs/dagre";
import type { Edge, Node } from "@xyflow/svelte";
import type { FlowDoc, FlowEdge, FlowNode } from "@waml/okf";

// ── Flow substrate rendering (behavioral substrates spec) ────────────────────
// A flow document is self-rendering: dagre lays the directed graph out at
// render time (relational, never coordinates — nothing is stored).

export function flowNodeSize(n: FlowNode): { width: number; height: number } {
  switch (n.kind) {
    case "initial":
    case "final":
      return { width: 36, height: 36 };
    case "decision":
    case "merge":
      return { width: 56, height: 56 };
    case "fork":
    case "join":
      return { width: 120, height: 10 };
    case "object":
      return { width: 160, height: 48 };
    default: {
      const internals = [n.entry, n.do, n.exit].filter(Boolean).length;
      return { width: 180, height: 48 + internals * 18 + (n.refines ? 18 : 0) };
    }
  }
}

/** UML edge label: `trigger [guard] / effect`; a decision default is `[else]`. */
export function transitionLabel(e: FlowEdge): string {
  const head = [e.trigger, e.guard ? `[${e.guard}]` : e.else ? "[else]" : undefined]
    .filter(Boolean)
    .join(" ");
  const eff = e.effect ? `/ ${e.effect}` : "";
  return [head, eff].filter(Boolean).join(" ").trim();
}

const KIND_TO_TYPE: Record<FlowNode["kind"], string> = {
  plain: "flowStep",
  object: "flowObject",
  initial: "flowControl",
  final: "flowControl",
  decision: "flowControl",
  merge: "flowControl",
  fork: "flowControl",
  join: "flowControl",
};

export function flowToRf(doc: FlowDoc): { nodes: Node[]; edges: Edge[] } {
  const g = new dagre.graphlib.Graph();
  g.setDefaultEdgeLabel(() => ({}));
  g.setGraph({ rankdir: "TB", nodesep: 50, ranksep: 70 });
  for (const n of doc.nodes) {
    const s = flowNodeSize(n);
    g.setNode(n.id, { width: s.width, height: s.height });
  }
  const local = new Set(doc.nodes.map((n) => n.id));
  for (const e of doc.edges) if (local.has(e.from) && local.has(e.to)) g.setEdge(e.from, e.to);
  dagre.layout(g);

  const nodes: Node[] = doc.nodes.map((n) => {
    const s = flowNodeSize(n);
    const pos = g.node(n.id);
    return {
      id: n.id,
      type: KIND_TO_TYPE[n.kind],
      position: { x: (pos?.x ?? 0) - s.width / 2, y: (pos?.y ?? 0) - s.height / 2 },
      data: { node: n, flavor: doc.flavor } as unknown as Record<string, unknown>,
      draggable: false,
      connectable: false,
      selectable: false,
    };
  });
  const edges: Edge[] = doc.edges
    .filter((e) => local.has(e.from) && local.has(e.to))
    .map((e, i) => ({
      id: `t${i}`,
      source: e.from,
      target: e.to,
      type: "transition",
      data: { label: transitionLabel(e), carries: e.carries } as unknown as Record<string, unknown>,
      selectable: false,
    }));
  return { nodes, edges };
}
```

- [ ] **Step 4: Run to verify pass**

Run: `pnpm --filter @waml/web test -- flowGraph`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add packages/web/src/canvas/flowGraph.ts packages/web/src/canvas/flowGraph.test.ts
git commit -m "feat(web): render-time dagre layout and UML labels for flow graphs"
```

---

### Task 14: Flow view components + switcher wiring

**Files:**
- Create: `packages/web/src/components/canvas/flow/FlowStepNode.svelte`
- Create: `packages/web/src/components/canvas/flow/FlowControlNode.svelte`
- Create: `packages/web/src/components/canvas/flow/FlowObjectNode.svelte`
- Create: `packages/web/src/components/canvas/flow/TransitionEdge.svelte`
- Create: `packages/web/src/components/canvas/flow/FlowView.svelte`
- Modify: `packages/web/src/components/canvas/CanvasInner.svelte` (switcher entries + view branch)
- Test: `packages/web/src/components/canvas/flow/FlowView.test.ts`

**Interfaces:**
- Consumes: `flowToRf` data shapes (Task 13); `getEdgeParams`/`NodeGeom` from `../floating` (existing).
- Produces: `<FlowView doc={FlowDoc} />`; CanvasInner derived `activeFlow: FlowDoc | undefined` and switcher entries for every flow doc. Task 20 extends the same branch for sequences.

- [ ] **Step 1: Write the failing test** — `packages/web/src/components/canvas/flow/FlowView.test.ts`:

```ts
import { describe, expect, it } from "vitest";
import { render } from "@testing-library/svelte";
import type { FlowDoc } from "@waml/okf";
import FlowView from "./FlowView.svelte";

const DOC: FlowDoc = {
  key: "m/lifecycle",
  title: "Order Lifecycle",
  flavor: "stateMachine",
  nodes: [
    { id: "initial", kind: "initial" },
    { id: "Placed", kind: "plain", entry: "reserveStock" },
    { id: "final", kind: "final" },
  ],
  edges: [
    { from: "initial", to: "Placed" },
    { from: "Placed", to: "final", trigger: "deliver" },
  ],
};

describe("FlowView", () => {
  it("renders every flow node with its internals", () => {
    const { getByText } = render(FlowView, { props: { doc: DOC } });
    expect(getByText("Placed")).toBeTruthy();
    expect(getByText("entry / reserveStock")).toBeTruthy();
  });
});
```

- [ ] **Step 2: Run to verify failure**

Run: `pnpm --filter @waml/web test -- FlowView`
Expected: FAIL — module not found.

- [ ] **Step 3: Implement**

`FlowStepNode.svelte`:

```svelte
<script lang="ts">
  import type { FlowFlavor, FlowNode } from "@waml/okf";

  let { data }: { data: { node: FlowNode; flavor: FlowFlavor } } = $props();
  const n = $derived(data.node);
  const internals = $derived(
    [
      n.entry ? `entry / ${n.entry}` : null,
      n.do ? `do / ${n.do}` : null,
      n.exit ? `exit / ${n.exit}` : null,
    ].filter((x): x is string => x != null),
  );
</script>

<!-- Action (activity) / state (state machine): rounded rect, optional internals. -->
<div
  class="w-[180px] select-none rounded-[12px] border-[1.5px] border-[#c8d2e0] bg-white px-3 py-[9px] text-center shadow-[0_2px_8px_rgba(15,23,42,0.05)]"
>
  <div class="text-[12.5px] font-semibold text-slate-800">{n.id}</div>
  {#if internals.length > 0}
    <div class="mt-1 border-t border-[#e2e8f0] pt-1 text-left text-[10.5px] leading-[18px] text-slate-600">
      {#each internals as row (row)}<div>{row}</div>{/each}
    </div>
  {/if}
  {#if n.refines}
    <div class="mt-1 text-left text-[10.5px] italic text-slate-500">↳ refines {n.refines}</div>
  {/if}
  {#if n.partition}
    <div class="mt-1 text-left text-[9.5px] uppercase tracking-wide text-slate-400">{n.partition}</div>
  {/if}
</div>
```

`FlowControlNode.svelte`:

```svelte
<script lang="ts">
  import type { FlowFlavor, FlowNode } from "@waml/okf";

  let { data }: { data: { node: FlowNode; flavor: FlowFlavor } } = $props();
  const n = $derived(data.node);
  const isKeywordOnly = $derived(n.id === n.kind);
</script>

<div class="relative flex flex-col items-center select-none">
  {#if n.kind === "initial"}
    <svg width="36" height="36"><circle cx="18" cy="18" r="10" fill="#334155" /></svg>
  {:else if n.kind === "final"}
    <svg width="36" height="36">
      <circle cx="18" cy="18" r="12" fill="none" stroke="#334155" stroke-width="2" />
      <circle cx="18" cy="18" r="7" fill="#334155" />
    </svg>
  {:else if n.kind === "decision" || n.kind === "merge"}
    <svg width="56" height="56"><path d="M28,4 L52,28 L28,52 L4,28 z" fill="#fff" stroke="#334155" stroke-width="2" /></svg>
  {:else}
    <!-- fork / join: synchronization bar -->
    <div class="h-[10px] w-[120px] rounded-[2px] bg-[#334155]"></div>
  {/if}
  {#if !isKeywordOnly}
    <div class="mt-1 max-w-[140px] text-center text-[11px] font-medium text-slate-700">{n.id}</div>
  {/if}
</div>
```

`FlowObjectNode.svelte`:

```svelte
<script lang="ts">
  import type { FlowFlavor, FlowNode } from "@waml/okf";

  let { data }: { data: { node: FlowNode; flavor: FlowFlavor } } = $props();
  const n = $derived(data.node);
</script>

<!-- Object/data node: plain rectangle, typed by a link when resolved. -->
<div
  class="flex h-[48px] w-[160px] items-center justify-center border-[1.5px] border-[#c8d2e0] bg-[#f8fafc] px-3 text-center select-none"
>
  <div class="text-[12px] font-semibold text-slate-800" class:underline={!!n.objectRef}>{n.id}</div>
</div>
```

`TransitionEdge.svelte`:

```svelte
<script lang="ts">
  import { BaseEdge, EdgeLabel, getSmoothStepPath, useInternalNode, type EdgeProps } from "@xyflow/svelte";
  import { getEdgeParams, type NodeGeom } from "../floating";

  let { id, source, target, data }: EdgeProps = $props();

  const sourceInternal = $derived(useInternalNode(source));
  const targetInternal = $derived(useInternalNode(target));
  const sourceNode = $derived(sourceInternal.current as NodeGeom | undefined);
  const targetNode = $derived(targetInternal.current as NodeGeom | undefined);
  const geometry = $derived(sourceNode && targetNode ? getEdgeParams(sourceNode, targetNode) : undefined);
  const d = $derived(data as { label?: string; carries?: string } | undefined);

  const edgePath = $derived.by(() => {
    if (!geometry) return undefined;
    const [p] = getSmoothStepPath({
      sourceX: geometry.sx,
      sourceY: geometry.sy,
      sourcePosition: geometry.sourcePos,
      targetX: geometry.tx,
      targetY: geometry.ty,
      targetPosition: geometry.targetPos,
      borderRadius: 8,
    });
    return p;
  });
</script>

{#if edgePath && geometry}
  <defs>
    <marker id="flow-arrow-{id}" markerWidth="12" markerHeight="12" refX="10" refY="6" orient="auto" markerUnits="userSpaceOnUse">
      <path d="M1,1 L10,6 L1,11" fill="none" stroke="#334155" stroke-width="1.5" />
    </marker>
  </defs>
  <BaseEdge {id} path={edgePath} markerEnd="url(#flow-arrow-{id})" style="stroke:#334155;stroke-width:1.6;" />
  {#if d?.label}
    <EdgeLabel
      x={(geometry.sx + geometry.tx) / 2}
      y={(geometry.sy + geometry.ty) / 2 - 10}
      class="nodrag nopan"
      style="background:rgba(255,255,255,0.9);border-radius:4px;padding:0 4px;font-size:10.5px;font-weight:600;color:#334155;white-space:nowrap;"
    >
      {d.label}
    </EdgeLabel>
  {/if}
{/if}
```

`FlowView.svelte`:

```svelte
<script lang="ts">
  import { SvelteFlow, SvelteFlowProvider, Background, BackgroundVariant, Controls, type Edge, type Node } from "@xyflow/svelte";
  import type { FlowDoc } from "@waml/okf";
  import { flowToRf } from "../../../canvas/flowGraph";
  import FlowStepNode from "./FlowStepNode.svelte";
  import FlowControlNode from "./FlowControlNode.svelte";
  import FlowObjectNode from "./FlowObjectNode.svelte";
  import TransitionEdge from "./TransitionEdge.svelte";

  let { doc }: { doc: FlowDoc } = $props();

  const nodeTypes = { flowStep: FlowStepNode, flowControl: FlowControlNode, flowObject: FlowObjectNode };
  const edgeTypes = { transition: TransitionEdge };

  let nodes = $state<Node[]>([]);
  let edges = $state<Edge[]>([]);
  $effect(() => {
    const rf = flowToRf(doc);
    nodes = rf.nodes;
    edges = rf.edges;
  });
</script>

<!-- A self-rendering behavior view: read-only, laid out at render time. Its own
     provider keeps this SvelteFlow instance isolated from the structure canvas. -->
<div class="h-full w-full" data-flow-view>
  <SvelteFlowProvider>
    <SvelteFlow bind:nodes bind:edges {nodeTypes} {edgeTypes} fitView nodesDraggable={false} nodesConnectable={false} zoomOnDoubleClick={false} deleteKey={null}>
      <Background variant={BackgroundVariant.Dots} gap={22} size={1.3} patternColor="#e2e6ec" />
      <Controls position="bottom-left" style="bottom:15px;left:15px;margin:0;" />
    </SvelteFlow>
  </SvelteFlowProvider>
</div>
```

`CanvasInner.svelte` — three edits:

1. Add the import next to the other component imports:

```ts
  import FlowView from "./flow/FlowView.svelte";
```

2. Replace the `const diagrams = $derived(effectiveDiagrams($model));` line with:

```ts
  // Behavior documents are both model and view — they join the switcher as
  // read-only views alongside curated Diagrams (behavioral substrates spec).
  const behaviorViews = $derived(
    ($model.flows ?? []).map((f) => ({ key: f.key, title: f.title, profile: "uml-domain", members: [] as string[] })),
  );
  const diagrams = $derived([...effectiveDiagrams($model), ...behaviorViews]);
  const activeFlow = $derived(($model.flows ?? []).find((f) => f.key === activeDiagramKey));
```

3. In the template, wrap the existing `<SvelteFlow …>…</SvelteFlow>` element (the one at ~line 602 that binds `rfNodes`/`rfEdges`, ends with `</SvelteFlow>` right before the `<!-- Empty canvas CTA -->` comment):

```svelte
      {#if activeFlow}
        <FlowView doc={activeFlow} />
      {:else}
      <SvelteFlow
        ... (existing element and children, unchanged) ...
      </SvelteFlow>
      {/if}
```

- [ ] **Step 4: Run to verify pass**

Run: `pnpm --filter @waml/web test`
Expected: PASS (FlowView test + all existing tests; the switcher change is additive — with no flow docs, `behaviorViews` is empty and everything behaves as before).

- [ ] **Step 5: Commit**

```bash
git add packages/web/src/components/canvas/flow packages/web/src/components/canvas/CanvasInner.svelte
git commit -m "feat(web): self-rendering flow view (activity/state machine) wired into the diagram switcher"
```

**Phase 2 done — activity + state machine diagrams ship end to end.**

---

# Phase 3 — Interaction substrate (`uml.Sequence`)

### Task 15: Sequence model enums + syntax types

**Files:**
- Modify: `crates/waml/src/model.rs` (MessageVerb, FragmentKind, Lifeline, SeqItem, SeqOperand, SequenceDoc, `Model.interactions`)
- Modify: `crates/waml/src/syntax.rs` (LifelineLine, ParsedMessage, SeqItemSyntax, SeqOperandSyntax, MessagesBlock, `Section::Lifelines`, `Section::Messages`)
- Test: inline in `model.rs`

**Interfaces:**
- Consumes: `LinkRef`, `Line<T>`, `ErrorNode` (Task 8 / existing).
- Produces (wire names — Task 19 mirrors):

```rust
MessageVerb: Calls|Sends|Replies|Creates|Destroys   // wire lowercase; parse()/as_str()
FragmentKind: Alt|Opt|Loop                          // wire lowercase; parse()/as_str()
Lifeline { title, alias, ref_→"ref" }
SeqItem (serde tag = "item", lowercase variants):
  Message { from, verb, to, signature } | Fragment { kind, operands }
SeqOperand { guard, items }                          // guard None = else operand
SequenceDoc { key, title, describes, lifelines, messages }
Model.interactions: Vec<SequenceDoc>                 // omitted when empty
syntax::LifelineLine { link: LinkRef, alias: Option<String>, line: usize, span: Option<(usize,usize)> }
syntax::ParsedMessage { from: String, verb: MessageVerb, to: String, signature: Option<String>, line: usize }
syntax::SeqItemSyntax = Message(ParsedMessage)
  | Fragment { kind: FragmentKind, operands: Vec<SeqOperandSyntax>, errors: Vec<ErrorNode>, line: usize }
syntax::SeqOperandSyntax { guard: Option<String>, items: Vec<Line<SeqItemSyntax>>, line: usize }
syntax::MessagesBlock { items: Vec<Line<SeqItemSyntax>> }
Section::Lifelines(Vec<Line<LifelineLine>>), Section::Messages(MessagesBlock)
```

- [ ] **Step 1: Write the failing tests** — in `model.rs` `tests`:

```rust
    #[test]
    fn message_verbs_and_fragment_kinds_round_trip() {
        for v in [MessageVerb::Calls, MessageVerb::Sends, MessageVerb::Replies, MessageVerb::Creates, MessageVerb::Destroys] {
            assert_eq!(MessageVerb::parse(v.as_str()), Some(v));
        }
        assert_eq!(MessageVerb::parse("shouts"), None);
        for k in [FragmentKind::Alt, FragmentKind::Opt, FragmentKind::Loop] {
            assert_eq!(FragmentKind::parse(k.as_str()), Some(k));
        }
        assert_eq!(FragmentKind::parse("par"), None, "par operands are deferred");
    }
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p waml model::tests`
Expected: FAIL — `MessageVerb` unknown (compile error).

- [ ] **Step 3: Implement**

`model.rs` — add after the flow types:

```rust
/// The message kind: fixes line and arrowhead (interaction substrate).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "lowercase"))]
pub enum MessageVerb {
    Calls,
    Sends,
    Replies,
    Creates,
    Destroys,
}

impl MessageVerb {
    pub fn as_str(self) -> &'static str {
        match self {
            MessageVerb::Calls => "calls",
            MessageVerb::Sends => "sends",
            MessageVerb::Replies => "replies",
            MessageVerb::Creates => "creates",
            MessageVerb::Destroys => "destroys",
        }
    }
    pub fn parse(s: &str) -> Option<MessageVerb> {
        match s {
            "calls" => Some(MessageVerb::Calls),
            "sends" => Some(MessageVerb::Sends),
            "replies" => Some(MessageVerb::Replies),
            "creates" => Some(MessageVerb::Creates),
            "destroys" => Some(MessageVerb::Destroys),
            _ => None,
        }
    }
}

/// Combined-fragment keyword. `par` is deferred (open question in the spec).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "lowercase"))]
pub enum FragmentKind {
    Alt,
    Opt,
    Loop,
}

impl FragmentKind {
    pub fn as_str(self) -> &'static str {
        match self {
            FragmentKind::Alt => "alt",
            FragmentKind::Opt => "opt",
            FragmentKind::Loop => "loop",
        }
    }
    pub fn parse(s: &str) -> Option<FragmentKind> {
        match s {
            "alt" => Some(FragmentKind::Alt),
            "opt" => Some(FragmentKind::Opt),
            "loop" => Some(FragmentKind::Loop),
            _ => None,
        }
    }
}

/// A sequence participant: IS a Class or Actor, referenced by link.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Lifeline {
    pub title: String,
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Option::is_none"))]
    pub alias: Option<String>,
    /// Resolved key of the classifier this lifeline is; None when unresolved.
    #[cfg_attr(feature = "serde", serde(rename = "ref", default, skip_serializing_if = "Option::is_none"))]
    pub ref_: Option<String>,
}

/// One operand of a combined fragment. `guard: None` = the `else` operand.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SeqOperand {
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Option::is_none"))]
    pub guard: Option<String>,
    pub items: Vec<SeqItem>,
}

/// One ordered interaction item: document order is time order.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(tag = "item", rename_all = "lowercase"))]
pub enum SeqItem {
    Message {
        from: String,
        verb: MessageVerb,
        to: String,
        #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Option::is_none"))]
        signature: Option<String>,
    },
    Fragment {
        kind: FragmentKind,
        operands: Vec<SeqOperand>,
    },
}

/// One sequence document: lifelines + ordered messages (model AND view).
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SequenceDoc {
    pub key: String,
    pub title: String,
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Option::is_none"))]
    pub describes: Option<String>,
    pub lifelines: Vec<Lifeline>,
    pub messages: Vec<SeqItem>,
}
```

Add to `struct Model`:

```rust
    /// Interaction-substrate behavior documents (uml.Sequence).
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Vec::is_empty"))]
    pub interactions: Vec<SequenceDoc>,
```

`syntax.rs` — add after the flow syntax types:

```rust
/// One `## Lifelines` bullet: `- [Title](./slug.md)[ as alias]`.
#[derive(Debug, Clone, PartialEq)]
pub struct LifelineLine {
    pub link: LinkRef,
    pub alias: Option<String>,
    pub line: usize,
    pub span: Option<(usize, usize)>,
}

/// One message bullet: `- <sender> <verb> <receiver>[: `signature`]`.
#[derive(Debug, Clone, PartialEq)]
pub struct ParsedMessage {
    pub from: String,
    pub verb: crate::model::MessageVerb,
    pub to: String,
    pub signature: Option<String>,
    pub line: usize,
}

/// One operand of a fragment as authored (`- when `g`` / `- else`).
#[derive(Debug, Clone, PartialEq)]
pub struct SeqOperandSyntax {
    /// None = the `else` operand.
    pub guard: Option<String>,
    pub items: Vec<Line<SeqItemSyntax>>,
    pub line: usize,
}

/// One `## Messages` item: a message, or a fragment owning operands.
/// `errors` preserves misplaced lines authored directly inside a fragment
/// (outside any operand) so serialization stays lossless.
#[derive(Debug, Clone, PartialEq)]
pub enum SeqItemSyntax {
    Message(ParsedMessage),
    Fragment {
        kind: crate::model::FragmentKind,
        operands: Vec<SeqOperandSyntax>,
        errors: Vec<ErrorNode>,
        line: usize,
    },
}

/// The ordered `## Messages` section. Document order is time order.
#[derive(Debug, Clone, PartialEq)]
pub struct MessagesBlock {
    pub items: Vec<Line<SeqItemSyntax>>,
}
```

Add to `enum Section`:

```rust
    /// A sequence document's participants.
    Lifelines(Vec<Line<LifelineLine>>),
    /// A sequence document's ordered messages.
    Messages(MessagesBlock),
```

- [ ] **Step 4: Run to verify pass**

Run: `cargo test -p waml`
Expected: PASS (new `Section` variants compile because every existing `match` on `Section` has a `_` arm or will be extended in Tasks 16–17 — verify: `parse.rs::diagnostics_of` and `build_node` use `_ => {}`; `serialize.rs::section_order`/`render_section` are exhaustive, so ADD the two arms there now as part of this task to keep the build green:)

In `serialize.rs` `section_order` (replacing the Task 9 version):

```rust
fn section_order(s: &Section) -> u8 {
    match s {
        Section::Body(_) => 0,
        Section::Attributes(_) => 1,
        Section::Values(_) => 2,
        Section::Relationships(_) => 3,
        Section::Notes(_) => 4,
        Section::Nodes(_) => 5,
        Section::Lifelines(_) => 6,
        Section::Messages(_) => 7,
        Section::Members(_) => 8,
        Section::Layout(_) => 9,
        Section::Unknown { .. } => 10,
    }
}
```

In `render_section`, add placeholder arms that Task 17 will replace with the real renderers — NO: placeholders are forbidden. Instead this task adds the real render arms, forward-declaring the Task 16 renderers is impossible — so **move the two `render_section` arms and the two renderers into Task 16's scope is also wrong ordering**. Resolution: this task adds the `Section` variants AND the two real renderers are added in Task 16 *in the same commit as the section arms*; to keep THIS task compiling, do **not** add the `Section::Lifelines/Messages` variants here — they move to Task 16. **Concretely: in this task, add only the model types (`model.rs`) and the plain syntax structs (`LifelineLine`, `ParsedMessage`, `SeqItemSyntax`, `SeqOperandSyntax`, `MessagesBlock`) — leave `enum Section` untouched.** Run `cargo test -p waml` → PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/waml/src/model.rs crates/waml/src/syntax.rs
git commit -m "feat(model): sequence-substrate model and syntax types (verbs, fragments, lifelines)"
```

---

### Task 16: Sequence grammar — lifeline/message lines, nested messages block, renderers, Section wiring

**Files:**
- Modify: `crates/waml/src/diagnostic.rs` (add `MalformedMessage`, `MalformedLifeline`)
- Modify: `crates/waml/src/syntax.rs` (add `Section::Lifelines`, `Section::Messages`)
- Modify: `crates/waml/src/grammar.rs` (parsers + renderers)
- Modify: `crates/waml/src/serialize.rs` (section order + render arms)
- Modify: `crates/waml/src/parse.rs` (`walk_section` + `diagnostics_of`)
- Test: inline in `grammar.rs` and `serialize.rs`

**Interfaces:**
- Consumes: Task 15's types.
- Produces: `grammar::parse_lifeline_line(&str) -> Result<LifelineLine, LineError>`; `grammar::parse_message_line(&str) -> Result<ParsedMessage, LineError>`; `grammar::parse_messages_block(content: &str, content_abs_start: usize, src: &str) -> MessagesBlock`; `grammar::render_lifeline_line(&LifelineLine) -> String`; `grammar::render_messages_block(&MessagesBlock) -> String` (heading included); `DiagCode::{MalformedMessage("malformed-message"), MalformedLifeline("malformed-lifeline")}`. Task 17/18 read `Section::Lifelines/Messages`.

- [ ] **Step 1: Write the failing tests** — append to `grammar.rs` `tests`:

```rust
    use crate::model::{FragmentKind, MessageVerb};
    use crate::syntax::SeqItemSyntax;

    #[test]
    fn parses_lifeline_lines() {
        let l = parse_lifeline_line("- [Order](./order.md) as order").unwrap();
        assert_eq!(l.link.slug, "order");
        assert_eq!(l.alias.as_deref(), Some("order"));
        let l = parse_lifeline_line("- [Customer](./customer.md)").unwrap();
        assert_eq!(l.alias, None);
        assert!(parse_lifeline_line("- Customer").is_err(), "a lifeline IS a link");
    }

    #[test]
    fn parses_message_lines() {
        let m = parse_message_line("- Customer calls order: `place(items)`").unwrap();
        assert_eq!(m.from, "Customer");
        assert_eq!(m.verb, MessageVerb::Calls);
        assert_eq!(m.to, "order");
        assert_eq!(m.signature.as_deref(), Some("place(items)"));
        let m = parse_message_line("- order replies Customer: `confirmation`").unwrap();
        assert_eq!(m.verb, MessageVerb::Replies);
        assert!(parse_message_line("- Customer shouts order").is_err());
        assert!(parse_message_line("- par").is_err(), "par is deferred");
    }

    #[test]
    fn parses_nested_fragments_in_messages_block() {
        let content = "- Customer calls order: `place(items)`\n- alt\n  - when `paid`\n    - order calls wh: `ship()`\n  - else\n    - order sends Customer: `paymentFailed()`\n- order replies Customer: `confirmation`";
        let block = parse_messages_block(content, 0, content);
        assert_eq!(block.items.len(), 3);
        let SeqItemSyntax::Fragment { kind, operands, .. } = block.items[1].parsed().unwrap() else {
            panic!("expected a fragment")
        };
        assert_eq!(*kind, FragmentKind::Alt);
        assert_eq!(operands.len(), 2);
        assert_eq!(operands[0].guard.as_deref(), Some("paid"));
        assert_eq!(operands[1].guard, None); // else
        let SeqItemSyntax::Message(m) = operands[0].items[0].parsed().unwrap() else { panic!() };
        assert_eq!(m.to, "wh");
    }

    #[test]
    fn messages_block_round_trips() {
        let content = "- Customer calls order: `place(items)`\n- alt\n  - when `paid`\n    - order calls wh: `ship()`\n  - else\n    - order sends Customer: `paymentFailed()`\n- order replies Customer: `confirmation`";
        let block = parse_messages_block(content, 0, content);
        let rendered = render_messages_block(&block);
        let body = rendered.strip_prefix("## Messages\n").unwrap();
        assert_eq!(parse_messages_block(body, 0, body), block);
        assert_eq!(body, content);
    }

    #[test]
    fn misplaced_operand_and_unknown_fragment_degrade_to_error_lines() {
        let content = "- when `paid`\n- par\n- Customer calls order";
        let block = parse_messages_block(content, 0, content);
        assert_eq!(block.items.len(), 3);
        assert!(block.items[0].parsed().is_none(), "operand outside a fragment is an error line");
        assert!(block.items[1].parsed().is_none(), "'par' is deferred and degrades");
        assert!(block.items[2].parsed().is_some());
    }
```

And in `serialize.rs` `tests`:

```rust
    #[test]
    fn sequence_document_serialize_is_a_semantic_fixpoint() {
        let src = "---\ntype: uml.Sequence\ntitle: Place Order\ndescribes: [Place Order](./place-order.md)\n---\n# Place Order\n\n## Lifelines\n- [Customer](./customer.md)\n- [Order](./order.md) as order\n- [Warehouse](./warehouse.md) as wh\n\n## Messages\n- Customer calls order: `place(items)`\n- alt\n  - when `paid`\n    - order calls wh: `ship()`\n  - else\n    - order sends Customer: `paymentFailed()`\n- order replies Customer: `confirmation`\n";
        let once = serialize_document(&parse_document(src));
        let twice = serialize_document(&parse_document(&once));
        assert_eq!(once, twice);
        assert!(once.contains("## Lifelines\n- [Customer](./customer.md)\n- [Order](./order.md) as order"));
        assert!(once.contains("- alt\n  - when `paid`\n    - order calls wh: `ship()`"));
    }
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p waml`
Expected: FAIL — `parse_lifeline_line` not found (compile error).

- [ ] **Step 3: Implement**

`diagnostic.rs` — add `MalformedMessage` (`"malformed-message"`) and `MalformedLifeline` (`"malformed-lifeline"`) variants + `as_str` arms (Error severity by default).

`syntax.rs` — add to `enum Section` (this now compiles because the renderers land in the same commit):

```rust
    /// A sequence document's participants.
    Lifelines(Vec<Line<LifelineLine>>),
    /// A sequence document's ordered messages.
    Messages(MessagesBlock),
```

`grammar.rs` — extend the `crate::syntax` import with `LifelineLine, MessagesBlock, ParsedMessage, SeqItemSyntax, SeqOperandSyntax`; add regexes:

```rust
static LIFELINE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^- \[([^\]]+)\]\(\./(.+?)\.md\)(?: as ([A-Za-z][A-Za-z0-9_]*))?$").unwrap());
static MESSAGE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^- (.+?) (calls|sends|replies|creates|destroys) (.+?)(?::\s*`([^`]+)`)?$").unwrap()
});
static SEQ_FRAGMENT_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^- (alt|opt|loop)$").unwrap());
static SEQ_OPERAND_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^- (?:when `([^`]+)`|else)$").unwrap());
```

Add the functions:

```rust
pub fn parse_lifeline_line(line: &str) -> Result<LifelineLine, LineError> {
    let trimmed = line.trim_end_matches('\r').trim();
    let m = LIFELINE_RE.captures(trimmed).ok_or_else(|| LineError {
        range: bullet_range(line),
        message: "malformed lifeline — expected '- [Title](./slug.md)[ as alias]' (a lifeline IS a Class or Actor, so it is a link)".to_string(),
    })?;
    Ok(LifelineLine {
        link: LinkRef { title: m[1].to_string(), slug: m[2].to_string() },
        alias: m.get(3).map(|x| x.as_str().to_string()),
        line: 0,
        span: None,
    })
}

fn message_error_message(line: &str) -> String {
    let first = line.trim_start_matches("- ").split_whitespace().next().unwrap_or("");
    if first == "par" {
        "'par' fragments are deferred — supported fragments are alt, opt, loop".to_string()
    } else {
        "malformed message — expected '<sender> <verb> <receiver>[: `signature`]' with verb one of calls/sends/replies/creates/destroys".to_string()
    }
}

pub fn parse_message_line(line: &str) -> Result<ParsedMessage, LineError> {
    let trimmed = line.trim_end_matches('\r').trim();
    let m = MESSAGE_RE.captures(trimmed).ok_or_else(|| LineError {
        range: bullet_range(line),
        message: message_error_message(trimmed),
    })?;
    Ok(ParsedMessage {
        from: m[1].trim().to_string(),
        verb: crate::model::MessageVerb::parse(&m[2]).expect("regex alternation is the closed verb set"),
        to: m[3].trim().to_string(),
        signature: m.get(4).map(|x| x.as_str().to_string()),
        line: 0,
    })
}

pub fn render_lifeline_line(l: &LifelineLine) -> String {
    match &l.alias {
        Some(a) => format!("- [{}](./{}.md) as {a}", l.link.title, l.link.slug),
        None => format!("- [{}](./{}.md)", l.link.title, l.link.slug),
    }
}

fn render_message_line(m: &ParsedMessage) -> String {
    match &m.signature {
        Some(sig) => format!("- {} {} {}: `{sig}`", m.from, m.verb.as_str(), m.to),
        None => format!("- {} {} {}", m.from, m.verb.as_str(), m.to),
    }
}

/// Parse the raw text under `## Messages`. Nesting is by indentation (two
/// spaces per level): a fragment owns operands one level deeper; an operand's
/// items nest one deeper again. Malformed/misplaced lines are preserved as
/// positioned error nodes — nothing is dropped.
pub fn parse_messages_block(content: &str, content_abs_start: usize, src: &str) -> MessagesBlock {
    use crate::model::FragmentKind;

    enum Open {
        Fragment { kind: FragmentKind, operands: Vec<SeqOperandSyntax>, errors: Vec<ErrorNode>, line: usize, level: usize },
        Operand { guard: Option<String>, items: Vec<Line<SeqItemSyntax>>, line: usize, level: usize },
    }
    fn level_of(o: &Open) -> usize {
        match o {
            Open::Fragment { level, .. } | Open::Operand { level, .. } => *level,
        }
    }
    fn close_one(stack: &mut Vec<Open>, top: &mut Vec<Line<SeqItemSyntax>>) {
        match stack.pop().expect("close_one on a non-empty stack") {
            Open::Operand { guard, items, line, .. } => match stack.last_mut() {
                Some(Open::Fragment { operands, .. }) => operands.push(SeqOperandSyntax { guard, items, line }),
                _ => unreachable!("an operand is only ever opened under a fragment"),
            },
            Open::Fragment { kind, operands, errors, line, .. } => {
                let item = Line::Parsed(SeqItemSyntax::Fragment { kind, operands, errors, line });
                match stack.last_mut() {
                    Some(Open::Operand { items, .. }) => items.push(item),
                    None => top.push(item),
                    Some(Open::Fragment { .. }) => unreachable!("a fragment is never opened under a fragment"),
                }
            }
        }
    }

    let mut top: Vec<Line<SeqItemSyntax>> = Vec::new();
    let mut stack: Vec<Open> = Vec::new();
    let mut fence: Option<char> = None;
    let mut offset = 0usize;

    for raw in content.split('\n') {
        let line_start = offset;
        offset += raw.len() + 1;
        let line = raw.trim_end_matches('\r');
        let t = line.trim_start();

        if let Some(marker) = fence {
            let delim = if marker == '`' { "```" } else { "~~~" };
            if t.starts_with(delim) {
                fence = None;
            }
            continue;
        }
        if t.starts_with("```") {
            fence = Some('`');
            continue;
        }
        if t.starts_with("~~~") {
            fence = Some('~');
            continue;
        }
        if t.is_empty() {
            continue;
        }

        let line_no = crate::parse::line_at(src, content_abs_start + line_start);
        let level = (line.len() - t.len()) / 2;
        while stack.last().map(|o| level_of(o) >= level).unwrap_or(false) {
            close_one(&mut stack, &mut top);
        }
        let in_fragment = matches!(stack.last(), Some(Open::Fragment { .. }));

        let mk_err = |code: DiagCode, message: String| ErrorNode {
            raw: raw.to_string(),
            line: line_no,
            span: bullet_range(raw),
            code,
            message,
        };

        if !t.starts_with("- ") {
            let e = mk_err(DiagCode::DroppableContent, crate::parse::DROPPABLE_MSG.to_string());
            match stack.last_mut() {
                Some(Open::Operand { items, .. }) => items.push(Line::Error(e)),
                Some(Open::Fragment { errors, .. }) => errors.push(e),
                None => top.push(Line::Error(e)),
            }
            continue;
        }

        if let Some(m) = SEQ_OPERAND_RE.captures(t) {
            if in_fragment {
                stack.push(Open::Operand {
                    guard: m.get(1).map(|x| x.as_str().to_string()),
                    items: vec![],
                    line: line_no,
                    level,
                });
            } else {
                let e = mk_err(
                    DiagCode::MalformedMessage,
                    "'when'/'else' operand outside an alt/opt/loop fragment".to_string(),
                );
                match stack.last_mut() {
                    Some(Open::Operand { items, .. }) => items.push(Line::Error(e)),
                    _ => top.push(Line::Error(e)),
                }
            }
            continue;
        }
        if let Some(m) = SEQ_FRAGMENT_RE.captures(t) {
            let kind = crate::model::FragmentKind::parse(&m[1]).expect("regex alternation is the closed set");
            if in_fragment {
                let e = mk_err(
                    DiagCode::MalformedMessage,
                    "a nested fragment must sit inside a 'when'/'else' operand".to_string(),
                );
                if let Some(Open::Fragment { errors, .. }) = stack.last_mut() {
                    errors.push(e);
                }
            } else {
                stack.push(Open::Fragment { kind, operands: vec![], errors: vec![], line: line_no, level });
            }
            continue;
        }
        match parse_message_line(t) {
            Ok(mut msg) => {
                msg.line = line_no;
                if in_fragment {
                    let e = mk_err(
                        DiagCode::MalformedMessage,
                        "expected a 'when `guard`' or 'else' operand before messages inside a fragment".to_string(),
                    );
                    if let Some(Open::Fragment { errors, .. }) = stack.last_mut() {
                        errors.push(e);
                    }
                } else {
                    let item = Line::Parsed(SeqItemSyntax::Message(msg));
                    match stack.last_mut() {
                        Some(Open::Operand { items, .. }) => items.push(item),
                        _ => top.push(item),
                    }
                }
            }
            Err(le) => {
                let e = mk_err(DiagCode::MalformedMessage, le.message);
                match stack.last_mut() {
                    Some(Open::Operand { items, .. }) => items.push(Line::Error(e)),
                    Some(Open::Fragment { errors, .. }) => errors.push(e),
                    None => top.push(Line::Error(e)),
                }
            }
        }
    }
    while !stack.is_empty() {
        close_one(&mut stack, &mut top);
    }
    MessagesBlock { items: top }
}

/// Render a messages block, `## Messages` heading included.
pub fn render_messages_block(block: &MessagesBlock) -> String {
    fn render_items(out: &mut String, items: &[Line<SeqItemSyntax>], depth: usize) {
        for it in items {
            out.push('\n');
            match it {
                Line::Error(e) => out.push_str(&e.raw),
                Line::Parsed(SeqItemSyntax::Message(m)) => {
                    out.push_str(&"  ".repeat(depth));
                    out.push_str(&render_message_line(m));
                }
                Line::Parsed(SeqItemSyntax::Fragment { kind, operands, errors, .. }) => {
                    out.push_str(&"  ".repeat(depth));
                    out.push_str(&format!("- {}", kind.as_str()));
                    for e in errors {
                        out.push('\n');
                        out.push_str(&e.raw);
                    }
                    for op in operands {
                        out.push('\n');
                        out.push_str(&"  ".repeat(depth + 1));
                        match &op.guard {
                            Some(g) => out.push_str(&format!("- when `{g}`")),
                            None => out.push_str("- else"),
                        }
                        render_items(out, &op.items, depth + 2);
                    }
                }
            }
        }
    }
    let mut out = String::from("## Messages");
    render_items(&mut out, &block.items, 0);
    out
}
```

`parse.rs` — in `walk_section`, add:

```rust
        "lifelines" => Section::Lifelines(walk_bullets(
            content, content_abs_start, src, DiagCode::MalformedLifeline,
            |line, ln| {
                crate::grammar::parse_lifeline_line(line).map(|mut l| {
                    l.line = ln;
                    l.span = Some(find_link_span(line, &l.link.title, &l.link.slug));
                    l
                })
            },
        )),
        "messages" => Section::Messages(crate::grammar::parse_messages_block(content, content_abs_start, src)),
```

In `diagnostics_of`, add:

```rust
            Section::Lifelines(v) => push_line_errors(v, &mut out),
            Section::Messages(block) => push_seq_errors(&block.items, &mut out),
```

and above `diagnostics_of` add:

```rust
fn push_seq_errors(items: &[crate::syntax::Line<crate::syntax::SeqItemSyntax>], out: &mut Vec<Diagnostic>) {
    for it in items {
        match it {
            crate::syntax::Line::Error(e) => {
                out.push(Diagnostic::new(e.code, e.message.clone(), "", e.line).with_span(e.span));
            }
            crate::syntax::Line::Parsed(crate::syntax::SeqItemSyntax::Fragment { operands, errors, .. }) => {
                for e in errors {
                    out.push(Diagnostic::new(e.code, e.message.clone(), "", e.line).with_span(e.span));
                }
                for op in operands {
                    push_seq_errors(&op.items, out);
                }
            }
            crate::syntax::Line::Parsed(crate::syntax::SeqItemSyntax::Message(_)) => {}
        }
    }
}
```

`serialize.rs` — replace `section_order` with the final numbering (shown in Task 15 Step 4's discussion):

```rust
fn section_order(s: &Section) -> u8 {
    match s {
        Section::Body(_) => 0,
        Section::Attributes(_) => 1,
        Section::Values(_) => 2,
        Section::Relationships(_) => 3,
        Section::Notes(_) => 4,
        Section::Nodes(_) => 5,
        Section::Lifelines(_) => 6,
        Section::Messages(_) => 7,
        Section::Members(_) => 8,
        Section::Layout(_) => 9,
        Section::Unknown { .. } => 10,
    }
}
```

In `render_section`, add:

```rust
        Section::Lifelines(lines) => {
            let body = lines
                .iter()
                .map(|l| match l {
                    Line::Parsed(x) => crate::grammar::render_lifeline_line(x),
                    Line::Error(e) => e.raw.clone(),
                })
                .collect::<Vec<_>>()
                .join("\n");
            format!("## Lifelines\n{body}")
        }
        Section::Messages(block) => crate::grammar::render_messages_block(block),
```

- [ ] **Step 4: Run to verify pass**

Run: `cargo test -p waml`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/waml/src/diagnostic.rs crates/waml/src/syntax.rs crates/waml/src/grammar.rs crates/waml/src/serialize.rs crates/waml/src/parse.rs
git commit -m "feat(parse): lossless ## Lifelines and nested ## Messages sections with fragments"
```

---

### Task 17: `SequenceDoc` build + validation

**Files:**
- Modify: `crates/waml/src/parse.rs` (`build_interactions`, `build_model`)
- Modify: `crates/waml/src/validate.rs` (lifeline/participant checks)
- Test: inline in both; wire pin in `crates/waml/tests/serde_shape.rs`

**Interfaces:**
- Consumes: Tasks 15–16; `resolve_describes` (Task 10).
- Produces: `Model.interactions` populated; participant handles = alias when declared, else lifeline title; unresolved participants kept verbatim + warned. Task 19 consumes the wire shape.

- [ ] **Step 1: Write the failing tests**

In `parse.rs` `model_tests`:

```rust
    #[test]
    fn builds_sequence_doc_with_resolved_lifelines_and_nested_messages() {
        use crate::model::{FragmentKind, MessageVerb, SeqItem};
        let b = vec![
            ("s/customer.md".into(), "---\ntype: uml.Actor\ntitle: Customer\n---\n# Customer\n".into()),
            ("s/order.md".into(), "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n".into()),
            ("s/place-order.md".into(),
             "---\ntype: uml.Sequence\ntitle: Place Order\ndescribes: [Order](./order.md)\n---\n# Place Order\n\n## Lifelines\n- [Customer](./customer.md)\n- [Order](./order.md) as order\n- [Warehouse](./warehouse.md) as wh\n\n## Messages\n- Customer calls order: `place(items)`\n- alt\n  - when `paid`\n    - order calls wh: `ship()`\n  - else\n    - order sends Customer: `paymentFailed()`\n- order replies Customer: `confirmation`\n".into()),
        ];
        let m = build_model(&b);
        assert_eq!(m.interactions.len(), 1);
        let s = &m.interactions[0];
        assert_eq!(s.key, "s/place-order");
        assert_eq!(s.describes.as_deref(), Some("s/order"));
        assert_eq!(s.lifelines.len(), 3);
        assert_eq!(s.lifelines[0].ref_.as_deref(), Some("s/customer"));
        assert_eq!(s.lifelines[1].alias.as_deref(), Some("order"));
        assert_eq!(s.lifelines[2].ref_, None, "unresolved lifeline degrades to link title only");
        assert_eq!(s.messages.len(), 3);
        let SeqItem::Message { from, verb, to, signature } = &s.messages[0] else { panic!() };
        assert_eq!((from.as_str(), *verb, to.as_str()), ("Customer", MessageVerb::Calls, "order"));
        assert_eq!(signature.as_deref(), Some("place(items)"));
        let SeqItem::Fragment { kind, operands } = &s.messages[1] else { panic!() };
        assert_eq!(*kind, FragmentKind::Alt);
        assert_eq!(operands.len(), 2);
        assert_eq!(operands[0].guard.as_deref(), Some("paid"));
    }
```

In `validate.rs` `tests`:

```rust
    #[test]
    fn flags_unknown_message_participant() {
        let b = vec![
            ("s/customer.md".into(), "---\ntype: uml.Actor\ntitle: Customer\n---\n# Customer\n".into()),
            ("s/seq.md".into(),
             "---\ntype: uml.Sequence\ntitle: S\n---\n# S\n\n## Lifelines\n- [Customer](./customer.md)\n\n## Messages\n- Customer calls Ghost: `x()`\n".into()),
        ];
        let d = validate(&b);
        let w = d.iter().find(|x| x.code == DiagCode::UnresolvedTarget && x.message.contains("Ghost")).unwrap();
        assert_eq!(w.severity, Severity::Warning);
        assert_eq!(w.line, 11);
    }
```

In `serde_shape.rs`:

```rust
#[test]
fn sequence_doc_json_matches_ts_field_names() {
    let b = vec![
        ("s/order.md".to_string(), "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n".to_string()),
        ("s/seq.md".to_string(),
         "---\ntype: uml.Sequence\ntitle: S\n---\n# S\n\n## Lifelines\n- [Order](./order.md) as order\n\n## Messages\n- order calls order: `tick()`\n- opt\n  - when `ready`\n    - order sends order: `go()`\n".to_string()),
    ];
    let m = build_model(&b);
    let v = serde_json::to_value(&m).unwrap();
    let s = &v["interactions"][0];
    assert_eq!(s["lifelines"][0]["ref"], "s/order");
    assert_eq!(s["lifelines"][0]["alias"], "order");
    assert_eq!(s["messages"][0]["item"], "message");
    assert_eq!(s["messages"][0]["verb"], "calls");
    assert_eq!(s["messages"][0]["signature"], "tick()");
    assert_eq!(s["messages"][1]["item"], "fragment");
    assert_eq!(s["messages"][1]["kind"], "opt");
    assert_eq!(s["messages"][1]["operands"][0]["guard"], "ready");
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p waml`
Expected: FAIL — `build_interactions` missing / `interactions` empty.

- [ ] **Step 3: Implement**

`parse.rs` — extend the `crate::model` import list with `Lifeline, MessageVerb, SeqItem, SeqOperand, SequenceDoc` (MessageVerb only if needed — it is not: drop it if unused). Add:

```rust
fn build_interactions(parsed: &[ParsedDoc], keyset: &HashSet<&str>) -> Vec<SequenceDoc> {
    use crate::syntax::SeqItemSyntax;
    let mut out = Vec::new();
    for p in parsed {
        if p.ty != ClassifierType::Behavior(BehaviorKind::Sequence) {
            continue;
        }
        let mut lifelines: Vec<Lifeline> = Vec::new();
        for s in &p.doc.sections {
            let Section::Lifelines(lines) = s else { continue };
            for l in lines.iter().filter_map(Line::parsed) {
                let resolved = crate::okf::resolve_href(&p.path, &l.link.slug);
                lifelines.push(Lifeline {
                    title: l.link.title.clone(),
                    alias: l.alias.clone(),
                    ref_: keyset.contains(resolved.as_str()).then_some(resolved),
                });
            }
        }
        // A participant token (alias, title, or link) canonicalizes to the
        // lifeline's handle: its alias when declared, else its title.
        // Unresolved tokens are kept verbatim (validate warns; render degrades).
        let handle_of = |token: &str| -> String {
            let name = match crate::grammar::parse_link_ref(token) {
                Some(l) => l.title,
                None => token.to_string(),
            };
            for l in &lifelines {
                if l.alias.as_deref() == Some(name.as_str()) || l.title == name {
                    return l.alias.clone().unwrap_or_else(|| l.title.clone());
                }
            }
            name
        };
        fn items_of(items: &[Line<crate::syntax::SeqItemSyntax>], handle_of: &dyn Fn(&str) -> String) -> Vec<SeqItem> {
            items
                .iter()
                .filter_map(Line::parsed)
                .map(|it| match it {
                    SeqItemSyntax::Message(m) => SeqItem::Message {
                        from: handle_of(&m.from),
                        verb: m.verb,
                        to: handle_of(&m.to),
                        signature: m.signature.clone(),
                    },
                    SeqItemSyntax::Fragment { kind, operands, .. } => SeqItem::Fragment {
                        kind: *kind,
                        operands: operands
                            .iter()
                            .map(|o| SeqOperand { guard: o.guard.clone(), items: items_of(&o.items, handle_of) })
                            .collect(),
                    },
                })
                .collect()
        }
        let mut messages = Vec::new();
        for s in &p.doc.sections {
            if let Section::Messages(block) = s {
                messages = items_of(&block.items, &handle_of);
            }
        }
        out.push(SequenceDoc {
            key: p.id.clone(),
            title: doc_title(p),
            describes: resolve_describes(p, keyset),
            lifelines,
            messages,
        });
    }
    out
}
```

In `build_model`, add `let interactions = build_interactions(&parsed, &keyset);` next to `build_flows` and change the literal to:

```rust
    Model { nodes, edges, diagrams, path, packages, flows, interactions }
```

`validate.rs` — add a `Section` arm in the per-doc loop (alongside the flow arm):

```rust
                Section::Messages(block) => {
                    use crate::syntax::SeqItemSyntax;
                    // Participant tokens must match a declared lifeline
                    // (alias or title). Collect the declared names first.
                    let mut names: HashSet<String> = HashSet::new();
                    for sec in &doc.sections {
                        if let Section::Lifelines(lines) = sec {
                            for l in lines.iter().filter_map(Line::parsed) {
                                names.insert(l.link.title.clone());
                                if let Some(a) = &l.alias {
                                    names.insert(a.clone());
                                }
                            }
                        }
                    }
                    fn check_items(
                        items: &[Line<SeqItemSyntax>],
                        names: &HashSet<String>,
                        path: &str,
                        diags: &mut Vec<Diagnostic>,
                    ) {
                        for it in items.iter().filter_map(Line::parsed) {
                            match it {
                                SeqItemSyntax::Message(m) => {
                                    for token in [&m.from, &m.to] {
                                        let name = match crate::grammar::parse_link_ref(token) {
                                            Some(l) => l.title,
                                            None => token.clone(),
                                        };
                                        if !names.contains(&name) {
                                            diags.push(Diagnostic::warn(
                                                DiagCode::UnresolvedTarget,
                                                format!("message participant '{name}' matches no lifeline"),
                                                path,
                                                m.line,
                                            ));
                                        }
                                    }
                                }
                                SeqItemSyntax::Fragment { operands, .. } => {
                                    for op in operands {
                                        check_items(&op.items, names, path, diags);
                                    }
                                }
                            }
                        }
                    }
                    check_items(&block.items, &names, path, &mut diags);
                }
```

Also extend `link`'s keyset comment: behavior docs remain excluded (done in Task 11) — lifeline links resolve against classifiers via `build_interactions`; validate's existing `UnresolvedTarget` machinery does not need a lifeline-link check because a missing lifeline target degrades to `ref: None` (view still renders); participants are the semantic anchor checked here.

- [ ] **Step 4: Run to verify pass**

Run: `cargo test -p waml`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/waml/src/parse.rs crates/waml/src/validate.rs crates/waml/tests/serde_shape.rs
git commit -m "feat(model): SequenceDoc resolution with lifeline handles and participant validation"
```

---

### Task 18: TS sequence types + pass-through

**Files:**
- Modify: `packages/okf/src/types.ts`
- Modify: `packages/core/src/state/overlay.ts`
- Test: `packages/core/src/state/overlay.test.ts`

**Interfaces:**
- Consumes: Task 17's wire JSON.
- Produces: TS `MessageVerb`, `FragmentKind`, `Lifeline`, `SeqOperand`, `SeqItem`, `SequenceDoc` exported from `@waml/okf`; `ModelGraph.interactions?: SequenceDoc[]`; `toModelGraph` copies `model.interactions ?? []`. Tasks 19–20 import these.

- [ ] **Step 1: Write the failing test** — append to `overlay.test.ts`:

```ts
import type { SequenceDoc } from "@waml/okf";

it("passes sequence docs through to the ModelGraph", () => {
  const seq: SequenceDoc = {
    key: "s/place-order",
    title: "Place Order",
    lifelines: [{ title: "Customer", ref: "s/customer" }, { title: "Order", alias: "order" }],
    messages: [
      { item: "message", from: "Customer", verb: "calls", to: "order", signature: "place(items)" },
      { item: "fragment", kind: "alt", operands: [{ guard: "paid", items: [] }, { items: [] }] },
    ],
  };
  const rust = { nodes: [], edges: [], diagrams: [], path: "", packages: [], interactions: [seq] };
  const g = toModelGraph(rust as never, emptyOverlay());
  expect(g.interactions).toEqual([seq]);
});
```

- [ ] **Step 2: Run to verify failure**

Run: `pnpm --filter @waml/core test -- overlay`
Expected: FAIL — `SequenceDoc` not exported.

- [ ] **Step 3: Implement**

`packages/okf/src/types.ts` — add after the flow types:

```ts
// ── Interaction substrate (uml.Sequence) ─────────────────────────────────────
// Ordered messages over linked lifelines; document order is time order.

export type MessageVerb = "calls" | "sends" | "replies" | "creates" | "destroys";

export type FragmentKind = "alt" | "opt" | "loop";

export interface Lifeline {
  title: string;
  alias?: string;
  /** Resolved key of the Class/Actor this lifeline is. */
  ref?: string;
}

/** One fragment operand; absent `guard` = the `else` operand. */
export interface SeqOperand {
  guard?: string;
  items: SeqItem[];
}

export type SeqItem =
  | { item: "message"; from: string; verb: MessageVerb; to: string; signature?: string }
  | { item: "fragment"; kind: FragmentKind; operands: SeqOperand[] };

export interface SequenceDoc {
  key: string;
  title: string;
  /** Resolved key of the entity this interaction describes. */
  describes?: string;
  lifelines: Lifeline[];
  messages: SeqItem[];
}
```

Add to `RustModel` (next to the `flows` field added in Task 12):

```ts
  /** Interaction-substrate behavior docs; omitted by serde when empty. */
  interactions?: SequenceDoc[];
```

In `toModelGraph`'s return statement, extend it again:

```ts
  return { nodes, edges, diagrams, path: model.path ?? "", packages, flows: model.flows ?? [], interactions: model.interactions ?? [] };
```

- [ ] **Step 4: Run to verify pass**

Run: `pnpm build:wasm && pnpm --filter @waml/core test && pnpm --filter @waml/web test`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add packages/okf/src/types.ts packages/core/src/state/overlay.ts packages/core/src/state/overlay.test.ts
git commit -m "feat(ts): SequenceDoc wire types and ModelGraph pass-through"
```

---

### Task 19: Sequence layout — pure deterministic geometry (no solver)

**Files:**
- Create: `packages/web/src/canvas/sequenceLayout.ts`
- Test: `packages/web/src/canvas/sequenceLayout.test.ts`

**Interfaces:**
- Consumes: `SequenceDoc`, `Lifeline`, `SeqItem` from `@waml/okf` (Task 18).
- Produces: `layoutSequence(doc: SequenceDoc): SequenceLayout` where
  `SequenceLayout = { lifelines: { key: string; handle: string; x: number }[]; rows: SeqRow[]; height: number; width: number }`
  `SeqRow = { kind: "message"; y: number; fromX: number; toX: number; item: Extract<SeqItem,{item:"message"}>; self: boolean } | { kind: "fragmentStart"|"fragmentEnd"|"operandDivider"; y: number; depth: number; label?: string; x0: number; x1: number }`.
  Document order fixes row Y; lifeline declaration order fixes column X — no constraint solving.

- [ ] **Step 1: Write the failing test** — `packages/web/src/canvas/sequenceLayout.test.ts`:

```ts
import { describe, expect, it } from "vitest";
import type { SequenceDoc } from "@waml/okf";
import { layoutSequence } from "./sequenceLayout";

const DOC: SequenceDoc = {
  key: "s/place-order",
  title: "Place Order",
  lifelines: [
    { title: "Customer", ref: "s/customer" },
    { title: "Order", alias: "order", ref: "s/order" },
    { title: "Warehouse", alias: "wh" },
  ],
  messages: [
    { item: "message", from: "Customer", verb: "calls", to: "order", signature: "place(items)" },
    {
      item: "fragment",
      kind: "alt",
      operands: [
        { guard: "paid", items: [{ item: "message", from: "order", verb: "calls", to: "wh", signature: "ship()" }] },
        { items: [{ item: "message", from: "order", verb: "sends", to: "Customer", signature: "paymentFailed()" }] },
      ],
    },
    { item: "message", from: "order", verb: "replies", to: "Customer" },
  ],
};

describe("layoutSequence", () => {
  it("places lifelines in declaration order and rows in document order", () => {
    const l = layoutSequence(DOC);
    expect(l.lifelines.map((x) => x.handle)).toEqual(["Customer", "order", "wh"]);
    expect(l.lifelines[0].x).toBeLessThan(l.lifelines[1].x);
    expect(l.lifelines[1].x).toBeLessThan(l.lifelines[2].x);

    const kinds = l.rows.map((r) => r.kind);
    expect(kinds[0]).toBe("message");
    expect(kinds[1]).toBe("fragmentStart");
    expect(kinds).toContain("operandDivider");
    expect(kinds[kinds.length - 2]).toBe("fragmentEnd");
    expect(kinds[kinds.length - 1]).toBe("message");

    // rows strictly increase in y (document order is time order)
    for (let i = 1; i < l.rows.length; i++) expect(l.rows[i].y).toBeGreaterThan(l.rows[i - 1].y);
  });

  it("resolves message endpoints to lifeline x positions by handle", () => {
    const l = layoutSequence(DOC);
    const first = l.rows.find((r) => r.kind === "message")! as Extract<(typeof l.rows)[number], { kind: "message" }>;
    const customerX = l.lifelines.find((x) => x.handle === "Customer")!.x;
    const orderX = l.lifelines.find((x) => x.handle === "order")!.x;
    expect(first.fromX).toBe(customerX);
    expect(first.toX).toBe(orderX);
    expect(first.self).toBe(false);
  });

  it("marks a message with equal endpoints as a self message", () => {
    const selfDoc: SequenceDoc = {
      ...DOC,
      messages: [{ item: "message", from: "order", verb: "calls", to: "order", signature: "validate()" }],
    };
    const l = layoutSequence(selfDoc);
    const row = l.rows[0] as Extract<(typeof l.rows)[number], { kind: "message" }>;
    expect(row.self).toBe(true);
  });
});
```

- [ ] **Step 2: Run to verify failure**

Run: `pnpm --filter @waml/web test -- sequenceLayout`
Expected: FAIL — module not found.

- [ ] **Step 3: Implement** — `packages/web/src/canvas/sequenceLayout.ts`:

```ts
import type { Lifeline, SeqItem, SeqOperand, SequenceDoc } from "@waml/okf";

// ── Sequence substrate layout (behavioral substrates spec) ───────────────────
// Purely deterministic: document order fixes row Y, lifeline declaration order
// fixes column X. No constraint solving — ordered lifelines/messages ARE the
// layout, per the spec's "self-rendering" design principle.

const LANE_WIDTH = 170;
const LANE_MARGIN = 90;
const ROW_HEIGHT = 46;
const FRAGMENT_HEADER_HEIGHT = 26;

export interface LaneLayout {
  key: string;
  handle: string;
  x: number;
}

export type SeqRow =
  | { kind: "message"; y: number; fromX: number; toX: number; item: Extract<SeqItem, { item: "message" }>; self: boolean }
  | { kind: "fragmentStart"; y: number; depth: number; label: string; x0: number; x1: number }
  | { kind: "fragmentEnd"; y: number; depth: number; x0: number; x1: number }
  | { kind: "operandDivider"; y: number; depth: number; label?: string; x0: number; x1: number };

export interface SequenceLayout {
  lifelines: LaneLayout[];
  rows: SeqRow[];
  width: number;
  height: number;
}

function laneHandle(l: Lifeline): string {
  return l.alias ?? l.title;
}

export function layoutSequence(doc: SequenceDoc): SequenceLayout {
  const lifelines: LaneLayout[] = doc.lifelines.map((l, i) => ({
    key: l.ref ?? laneHandle(l),
    handle: laneHandle(l),
    x: LANE_MARGIN + i * LANE_WIDTH,
  }));
  const xOf = (handle: string): number => lifelines.find((l) => l.handle === handle)?.x ?? LANE_MARGIN;
  // A fragment spans every lane touched by messages inside it (min..max),
  // padded so its frame clears the endpoints.
  const bounds = (items: SeqItem[]): [number, number] => {
    let lo = Infinity;
    let hi = -Infinity;
    for (const it of items) {
      if (it.item === "message") {
        lo = Math.min(lo, xOf(it.from), xOf(it.to));
        hi = Math.max(hi, xOf(it.from), xOf(it.to));
      } else {
        for (const op of it.operands) {
          const [a, b] = bounds(op.items);
          lo = Math.min(lo, a);
          hi = Math.max(hi, b);
        }
      }
    }
    return lo === Infinity ? [LANE_MARGIN, LANE_MARGIN] : [lo, hi];
  };

  const rows: SeqRow[] = [];
  let y = 60;

  const walk = (items: SeqItem[], depth: number): void => {
    for (const it of items) {
      if (it.item === "message") {
        const fromX = xOf(it.from);
        const toX = xOf(it.to);
        rows.push({ kind: "message", y, fromX, toX, item: it, self: fromX === toX });
        y += ROW_HEIGHT;
      } else {
        const [lo, hi] = bounds([it]);
        const x0 = lo - 30 - depth * 12;
        const x1 = hi + 30 + depth * 12;
        rows.push({ kind: "fragmentStart", y, depth, label: it.kind, x0, x1 });
        y += FRAGMENT_HEADER_HEIGHT;
        it.operands.forEach((op: SeqOperand, i: number) => {
          if (i > 0) {
            rows.push({ kind: "operandDivider", y, depth, label: op.guard, x0, x1 });
            y += 20;
          }
          walk(op.items, depth + 1);
        });
        rows.push({ kind: "fragmentEnd", y, depth, x0, x1 });
        y += 14;
      }
    }
  };
  walk(doc.messages, 0);

  const width = lifelines.length > 0 ? Math.max(...lifelines.map((l) => l.x)) + LANE_MARGIN : LANE_MARGIN * 2;
  return { lifelines, rows, width, height: y + 40 };
}
```

- [ ] **Step 4: Run to verify pass**

Run: `pnpm --filter @waml/web test -- sequenceLayout`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add packages/web/src/canvas/sequenceLayout.ts packages/web/src/canvas/sequenceLayout.test.ts
git commit -m "feat(web): deterministic sequence-diagram layout — document order is time order"
```

---

### Task 20: Sequence view component + switcher wiring

**Files:**
- Create: `packages/web/src/components/canvas/sequence/SequenceView.svelte`
- Modify: `packages/web/src/components/canvas/CanvasInner.svelte`
- Test: `packages/web/src/components/canvas/sequence/SequenceView.test.ts`

**Interfaces:**
- Consumes: `layoutSequence` (Task 19); `SequenceDoc` (Task 18).
- Produces: `<SequenceView doc={SequenceDoc} />`; CanvasInner derived `activeSequence`; both `activeFlow` and `activeSequence` branch the canvas body alongside the structure `SvelteFlow`.

- [ ] **Step 1: Write the failing test** — `packages/web/src/components/canvas/sequence/SequenceView.test.ts`:

```ts
import { describe, expect, it } from "vitest";
import { render } from "@testing-library/svelte";
import type { SequenceDoc } from "@waml/okf";
import SequenceView from "./SequenceView.svelte";

const DOC: SequenceDoc = {
  key: "s/place-order",
  title: "Place Order",
  lifelines: [{ title: "Customer" }, { title: "Order", alias: "order" }],
  messages: [
    { item: "message", from: "Customer", verb: "calls", to: "order", signature: "place(items)" },
    { item: "message", from: "order", verb: "replies", to: "Customer", signature: "confirmation" },
  ],
};

describe("SequenceView", () => {
  it("renders every lifeline handle and message signature", () => {
    const { getByText } = render(SequenceView, { props: { doc: DOC } });
    expect(getByText("Customer")).toBeTruthy();
    expect(getByText("order")).toBeTruthy();
    expect(getByText("place(items)")).toBeTruthy();
    expect(getByText("confirmation")).toBeTruthy();
  });
});
```

- [ ] **Step 2: Run to verify failure**

Run: `pnpm --filter @waml/web test -- SequenceView`
Expected: FAIL — module not found.

- [ ] **Step 3: Implement** — `packages/web/src/components/canvas/sequence/SequenceView.svelte`:

```svelte
<script lang="ts">
  import type { SequenceDoc } from "@waml/okf";
  import { layoutSequence, type SeqRow } from "../../../canvas/sequenceLayout";

  let { doc }: { doc: SequenceDoc } = $props();
  const layout = $derived(layoutSequence(doc));

  // Solid+filled = calls (sync); solid+open = sends (async);
  // dashed+open = replies; dashed→new lifeline = creates; →✕ = destroys.
  const dashed = (r: Extract<SeqRow, { kind: "message" }>) =>
    r.item.verb === "replies" || r.item.verb === "creates";
  const filled = (r: Extract<SeqRow, { kind: "message" }>) => r.item.verb === "calls";
</script>

<!-- Self-rendering interaction view: read-only, plain SVG. Ordered lifelines
     and document-ordered messages ARE the layout — no solver is involved. -->
<div class="h-full w-full overflow-auto bg-white" data-sequence-view>
  <svg width={layout.width} height={layout.height} class="block">
    <defs>
      <marker id="seq-arrow-filled" markerWidth="12" markerHeight="12" refX="9" refY="5" orient="auto">
        <path d="M0,0 L10,5 L0,10 z" fill="#334155" />
      </marker>
      <marker id="seq-arrow-open" markerWidth="12" markerHeight="12" refX="9" refY="5" orient="auto">
        <path d="M0,0 L10,5 L0,10" fill="none" stroke="#334155" stroke-width="1.5" />
      </marker>
    </defs>

    {#each layout.lifelines as lane (lane.key)}
      <line x1={lane.x} y1={44} x2={lane.x} y2={layout.height - 10} stroke="#cbd5e1" stroke-width="1.5" stroke-dasharray="4 3" />
      <rect x={lane.x - 60} y={10} width="120" height="30" rx="4" fill="#f8fafc" stroke="#c8d2e0" stroke-width="1.5" />
      <text x={lane.x} y={30} text-anchor="middle" font-size="12" font-weight="600" fill="#1e293b">{lane.handle}</text>
    {/each}

    {#each layout.rows as row (row.y + row.kind)}
      {#if row.kind === "message"}
        {#if row.self}
          <path d={`M${row.fromX},${row.y} h30 v18 h-30`} fill="none" stroke="#334155" stroke-width="1.5" stroke-dasharray={dashed(row) ? "5 3" : undefined} marker-end={filled(row) ? "url(#seq-arrow-filled)" : "url(#seq-arrow-open)"} />
        {:else}
          <line x1={row.fromX} y1={row.y} x2={row.toX} y2={row.y} stroke="#334155" stroke-width="1.5" stroke-dasharray={dashed(row) ? "5 3" : undefined} marker-end={filled(row) ? "url(#seq-arrow-filled)" : "url(#seq-arrow-open)"} />
        {/if}
        {#if row.item.signature}
          <text x={(row.fromX + row.toX) / 2} y={row.y - 6} text-anchor="middle" font-size="11" fill="#334155">{row.item.signature}</text>
        {/if}
      {:else if row.kind === "fragmentStart"}
        <rect x={row.x0} y={row.y} width={row.x1 - row.x0} height={layout.height - row.y - 20} fill="none" stroke="#94a3b8" stroke-width="1.2" />
        <path d={`M${row.x0},${row.y} h34 v14 l-8,8 h-26 z`} fill="#eef2f7" stroke="#94a3b8" stroke-width="1.2" />
        <text x={row.x0 + 6} y={row.y + 15} font-size="10.5" font-weight="700" fill="#475569">{row.label}</text>
      {:else if row.kind === "operandDivider"}
        <line x1={row.x0} y1={row.y} x2={row.x1} y2={row.y} stroke="#94a3b8" stroke-width="1" stroke-dasharray="3 3" />
        {#if row.label}<text x={row.x0 + 6} y={row.y + 13} font-size="10" font-style="italic" fill="#64748b">[{row.label}]</text>{/if}
      {/if}
    {/each}
  </svg>
</div>
```

`CanvasInner.svelte` — three edits, layered on Task 14's:

1. Add the import next to `FlowView`:

```ts
  import SequenceView from "./sequence/SequenceView.svelte";
```

2. Extend the switcher/derived block:

```ts
  const sequenceViews = $derived(
    ($model.interactions ?? []).map((s) => ({ key: s.key, title: s.title, profile: "uml-domain", members: [] as string[] })),
  );
  const diagrams = $derived([...effectiveDiagrams($model), ...behaviorViews, ...sequenceViews]);
  const activeSequence = $derived(($model.interactions ?? []).find((s) => s.key === activeDiagramKey));
```

(This replaces the single-line `diagrams` derivation from Task 14 with the three-way union.)

3. Extend the template branch from Task 14:

```svelte
      {#if activeFlow}
        <FlowView doc={activeFlow} />
      {:else if activeSequence}
        <SequenceView doc={activeSequence} />
      {:else}
      <SvelteFlow
        ... (existing element and children, unchanged) ...
      </SvelteFlow>
      {/if}
```

- [ ] **Step 4: Run to verify pass**

Run: `pnpm --filter @waml/web test`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add packages/web/src/components/canvas/sequence packages/web/src/components/canvas/CanvasInner.svelte
git commit -m "feat(web): self-rendering sequence view wired into the diagram switcher"
```

**Phase 3 done — sequence diagrams ship end to end.**

---

### Task 21: LSP diagnostics — confirm free coverage with an end-to-end regression test

**Files:**
- Modify: `crates/waml-cli/tests/lsp_e2e.rs`

**Interfaces:**
- Consumes: `validate()` (Tasks 3, 11, 17 — already wired into `Workspace::publish_all()`, `crates/waml-cli/src/lsp/server.rs`, unmodified).
- Produces: nothing new — this task is a regression guard proving the LSP needs no substrate-specific code, since `Backend::did_open`/`did_change` already re-run whole-document `validate()` over every open file regardless of `ClassifierType`.

- [ ] **Step 1: Write the failing test** — append to `lsp_e2e.rs` (mirror the file's existing `did_open` test helper/setup pattern — `start_server`/`open_doc`/`await_diagnostics` or equivalent already used by the neighboring class-diagram test):

```rust
#[tokio::test]
async fn publishes_diagnostics_for_a_malformed_flow_bullet_with_no_extra_wiring() {
    let (mut client, _tmp) = start_server().await;
    open_doc(
        &mut client,
        "f/a.md",
        "---\ntype: uml.Activity\ntitle: A\n---\n# A\n\n## Nodes\n\n### Ship\n- goes to Deliver\n",
    )
    .await;
    let diags = await_diagnostics(&mut client, "f/a.md").await;
    assert!(diags.iter().any(|d| d.message.contains("unrecognized flow bullet")));
}
```

(Match the exact helper names already present in `lsp_e2e.rs`; if named differently, use the file's actual helpers verbatim — the assertion and fixture text are what matters.)

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p waml-cli --test lsp_e2e`
Expected: FAIL until Phase 2 (Tasks 7–11) lands; once those are merged this test passes with zero LSP code changes, which is the point being proven.

- [ ] **Step 3: Implement**

No production code changes. This step only runs the test after Phase 2/3 are complete to confirm the claim.

- [ ] **Step 4: Run to verify pass**

Run: `cargo test -p waml-cli --test lsp_e2e`
Expected: PASS — `Workspace`'s existing `validate()` call already covers flow/interaction diagnostics; no new capability, no completion provider needed (per the spec's scope: diagnostics only).

- [ ] **Step 5: Commit**

```bash
git add crates/waml-cli/tests/lsp_e2e.rs
git commit -m "test(lsp): regression-prove flow/interaction diagnostics need no LSP-specific wiring"
```

---

### Task 22: Fold the BNF into `docs/uaml-spec.md`

**Files:**
- Modify: `docs/uaml-spec.md`

**Interfaces:**
- Consumes: none (documentation only).
- Produces: the canonical grammar reference gains three new sections, closing the spec's own "Open questions — BNF" item.

- [ ] **Step 1: Draft the addition**

Append a new top-level section after the existing Relationships BNF section (matching the doc's existing heading style and code-fence convention), and add the two verbs to the existing Relationships verb table row:

```markdown
## Behavioral substrates

Three additions extend the structure grammar above and introduce two new
self-rendering substrates. Full rationale and worked examples:
`docs/superpowers/specs/2026-07-11-uaml-behavioral-substrates-design.md`.

### Structure tier additions

Two new metaclasses join the closed set: `uml.Actor`, `uml.UseCase`. Two new
dependency verbs join the Relationships grammar:

```
<rel-verb>      ::= "associates" | "aggregates" | "composes" | "specializes"
                   | "implements" | "depends" | "includes" | "extends" | "annotates"
```

`includes` and `extends` are dependencies: no ends, rendered dashed with an
open arrow and a `«include»` / `«extend»` label. Context rule: `associates`
ends (`: <near> to <far>`) are required between two classifiers, and
**optional** when either end is an `uml.Actor` or `uml.UseCase` (a bare
`associates` there is a communication link). A system boundary is a `frame`
group in a Diagram's `## Members` section — no new metaclass.

### Flow substrate (`uml.Activity`, `uml.StateMachine`)

One document is one directed graph — self-rendering, no `## Layout`. Optional
frontmatter `describes: [Title](./slug.md)` links the flow to the entity it
behaviorizes. `## Nodes` holds one `###` heading per vertex:

```
flow-heading    ::= "###" SP node-kind? SP node-identity
node-kind       ::= "initial" | "final" | "decision" | "merge"
                   | "fork" | "join" | "object" link
node-identity   ::= text                      ; heading text minus the keyword;
                                               ; an "object" node's identity is
                                               ; its link title
```

Each node owns zero or more bullets:

```
flow-bullet     ::= transition | internal | refines | partition
transition      ::= "-" SP ("on" SP expr SP)? (("when" SP expr) | "else")? SP
                     "transitions to" SP target
                     (SP "carries" SP link)?
                     (":" SP expr)?
target          ::= local-name | link
internal        ::= "-" SP ("entry" | "do" | "exit") ":" SP expr
refines         ::= "-" SP "refines" SP link
partition       ::= "-" SP "partition:" SP text
expr            ::= "`" text "`"               ; opaque to the model
link            ::= "[" text "](" "./" slug ".md" ")"
```

`transitions` is the one edge verb for both flow flavors. Guards are
delimited by the word `when`, never `[...]`. A trailing `#### Notes` under a
node is a plain bulleted list, same grammar as a classifier's `## Notes`.

### Interaction substrate (`uml.Sequence`)

One document is one ordered interaction — self-rendering. Optional
`describes:` as above. `## Lifelines` declares participants (order fixes the
diagram's columns); `## Messages` is the ordered list (document order is time
order):

```
lifeline-line   ::= "-" SP link (SP "as" SP alias)?
message-line    ::= "-" SP participant SP msg-verb SP participant
                     (":" SP expr)?
msg-verb        ::= "calls" | "sends" | "replies" | "creates" | "destroys"
participant     ::= local-name | link                ; resolves to a lifeline
                                                       ; by alias or title
fragment        ::= "-" SP frag-kind
                     ( operand )+
frag-kind       ::= "alt" | "opt" | "loop"
operand         ::= "-" SP ("when" SP expr | "else")
                     ( message-line | fragment )+     ; indented one level
                                                       ; deeper than the operand
```

`calls` (sync) renders solid line + filled arrowhead; `sends` (async) solid +
open arrowhead; `replies` dashed + open arrowhead; `creates` dashed arrow to a
new lifeline; `destroys` an arrow ending in `✕`. Execution bars are derived
from call/reply pairing — there is no syntax for them. `par` operands,
self/found/lost messages, gates, and coregions are not yet supported.
```

- [ ] **Step 2: Verify**

Run: `pnpm --filter @waml/web check` is unrelated; for a docs-only change, sanity-check locally that the new fenced code blocks are balanced and the file still renders as valid Markdown:

Run: `node -e "const fs=require('fs');const s=fs.readFileSync('docs/uaml-spec.md','utf8');const n=(s.match(/```/g)||[]).length;if(n%2!==0){console.error('unbalanced fences: '+n);process.exit(1)}console.log('ok, '+n+' fence markers')"`
Expected: `ok, <even number> fence markers`.

- [ ] **Step 3: Commit**

```bash
git add docs/uaml-spec.md
git commit -m "docs: fold behavioral-substrates BNF into the canonical grammar reference"
```

---

## Self-Review

**Spec coverage checklist** (against `docs/superpowers/specs/2026-07-11-uaml-behavioral-substrates-design.md`):

- Two-tier decision (entity vs behavior) — behavior docs excluded from classifier nodes/keyset, routed to `Model.flows`/`Model.interactions` (Task 7, 10, 11, 17). Covered.
- `uml.Actor` / `uml.UseCase` metaclasses, renderers, palette — Tasks 1, 4, 5. Covered.
- `includes` / `extends` verbs, dashed + guillemet rendering — Tasks 1, 2, 4, 6. Covered.
- `associates` ends-optional on actor↔use-case, required between classifiers (context rule) — Tasks 2, 3. Covered.
- System boundary = `frame` group in `## Members` — explicitly noted as "no new metaclass" in Task 22's doc text; no code task needed since `frame` grouping is existing `## Members` heading-forest behavior. Covered by explanation, not by new code (correctly — nothing to add).
- Flow: `uml.Activity` / `uml.StateMachine` one-grammar-both-flavors, closed node-kind keyword set, `### ` headings, identity = heading minus keyword, `transitions` grammar (trigger/guard/else/target/carries/effect), `entry`/`do`/`exit`, `refines`, `partition:`, `#### Notes` — Tasks 7–14. Covered.
- Interaction: `## Lifelines` (link + optional alias), `## Messages` ordered = time order, verb table (calls/sends/replies/creates/destroys with exact line/arrowhead styles), combined fragments (`alt`/`opt`/`loop`) owning `when`/`else` operands with nested messages, execution bars auto-derived (no syntax added, confirmed no execution-bar grammar exists anywhere in Tasks 15–20) — Tasks 15–20. Covered.
- Surface grammar markers (bare word / `` `backtick` `` / `[link]`) — enforced throughout: guards and effects are always backticked in the grammar (Task 8's `FLOW_TRANSITION_RE`, Task 16's message signature), targets/carries/refines/describes/lifelines are always links, node/lifeline identities are bare words. Covered.
- Rendering dispatch for both new self-rendering substrates at render time (never stored coordinates) — Task 13 (dagre) and Task 19 (deterministic SVG layout), both computed inside `$effect`/`$derived` on every render. Covered.
- Diagram switcher includes flow and sequence docs alongside curated Diagrams — Task 14, Task 20. Covered.
- LSP gets equivalent diagnostic treatment with no bespoke wiring — Task 21 proves this via regression test; `Workspace::publish_all()` already calls whole-document `validate()` regardless of `ClassifierType`. Covered.
- BNF drafted and folded into `docs/uaml-spec.md` — Task 22. Covered.

**Deferred-scope check** (must be **absent** from every task above): `par` operand grammar — not implemented (Task 16's `SEQ_FRAGMENT_RE` only matches `alt|opt|loop`; `FragmentKind::parse` returns `None` for `"par"`, degrading it to a preserved error line, per spec). Self/found/lost messages, gates, coregions — no message-origin/destination variant beyond binary `participant verb participant` exists anywhere. Swimlane/partition detail beyond the sketch — `partition:` is a single opaque string field, no swimlane geometry. Object-flow pins and class-diagram object-flow rendering — `carries` only appears on flow transitions, never added to `RelationshipKind` or `RelEdge.svelte`. History pseudostates, deferred events, entry/exit points on composite states, cross-boundary transitions into `refines`d submachines — none of these appear in `FlowNodeKind`, `FlowBullet`, or the flow grammar. Edge `as <name>` labels — the flow `transitions` grammar has no `as` clause (only lifelines do, per spec, for aliasing). Confirmed: no deferred item leaked into any task.

**Placeholder scan:** re-read every task's code blocks — no `TODO`, `TBD`, `unimplemented!()`, `// ...`, or "similar to Task N" shorthand appears in any Create/Modify code block; every function body, regex, and Svelte template above is complete and independently compilable/runnable given its stated imports. One phrasing note self-corrected during drafting: Task 15 originally risked adding `Section::Lifelines`/`Section::Messages` variants without their renderers in the same commit (would have broken the `render_section` exhaustive match); resolved by scoping Task 15 to model/plain-syntax types only and moving the `Section` variants + renderers together into Task 16, verified both tasks compile standalone in sequence.

**Type-consistency check (Rust wire to TypeScript):**
- `FlowNodeKind`/`MessageVerb`/`FragmentKind`: Rust `serde(rename_all = "lowercase")` on unit variants versus TS lowercase string-literal unions — matches for every variant (`initial|final|decision|merge|fork|join|object|plain`, `calls|sends|replies|creates|destroys`, `alt|opt|loop`).
- `FlowFlavor`: Rust `serde(rename_all = "camelCase")` → `Activity` "activity", `StateMachine` "stateMachine" — matches TS `"activity" | "stateMachine"`.
- `FlowNode`/`FlowEdge` field renames: `object_ref`→`objectRef`, `do_`→`do`, `to_ref`→`toRef`, `is_else`→`else` — all four appear identically renamed on the TS `FlowNode`/`FlowEdge` interfaces (Task 12).
- `Lifeline.ref_`→`ref` — matches TS `Lifeline.ref` (Task 18).
- `SeqItem` tag: Rust `#[serde(tag = "item", rename_all = "lowercase")]` on `Message`/`Fragment` → wire `item: "message"|"fragment"` — matches the TS discriminated union's `item` field exactly (Task 18); confirmed against Task 17's `serde_shape.rs` assertions (`s["messages"][0]["item"] == "message"`, `s["messages"][1]["item"] == "fragment"`).
- `Model.flows` / `Model.interactions`: both `skip_serializing_if = "Vec::is_empty"` on the Rust side, and both consumed as `model.flows ?? []` / `model.interactions ?? []` on the TS side (Tasks 12, 18) — no undefined-vs-missing-key mismatch.
- All new `RelationshipKind` values (`includes`, `extends`) appear in both `RELATIONSHIP_KINDS` (TS const array, Task 4) and the Rust `RelationshipKind::as_str`/`parse` pair (Task 1) with identical strings.

No fixes were needed beyond the one documented self-correction above; the plan is internally consistent and ready to execute.
