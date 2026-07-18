# Sequence Flat Model Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Flatten the runtime `SequenceDoc` into the unified nodes+edges shape — lifelines/fragments/operands become `SeqNode`s, messages become ordered, interaction-local `SeqEdge`s, and containment is preserved by id reference — without touching the on-disk (markdown) storage format.

**Architecture:** `SequenceDoc` is the *runtime* interaction model built by `build_interactions` (`crates/waml/src/parse.rs`) from the nested `## Lifelines`/`## Messages` *syntax* layer (`SeqItemSyntax`/`MessagesBlock`/`LifelineLine`). Storage (the syntax layer + grammar/serialize round-trip) is the source of truth and stays nested — this is exactly the spec's "storage and runtime are mostly, but not always, 1:1" case (§9, §6). Only the runtime model is reshaped: `Lifeline`/`SeqItem`/`SeqOperand` are replaced by `SeqNode` (tagged `lifeline`/`fragment`/`operand`), `SeqEdge` (a message), and `SeqChild` (an ordered `message`/`fragment` ref). The flat model is emitted verbatim to TypeScript through Tsify, so the wasm bindings regenerate and the Svelte sequence renderer rebuilds from the flat shape.

**Tech Stack:** Rust (`waml` crate; `cargo`, serde internally-tagged enums, `tsify_next`/`wasm-bindgen` codegen), TypeScript monorepo (`packages/`, `pnpm`, Svelte 5 runes, Vitest).

## Assumes the behavior slice has ALREADY landed

This plan is built SERIALLY, **on top of** the `2026-07-18-behavior-model-view-split` slice (its predecessor on `main`). It was originally drafted in parallel, blind to that slice; every task below has been rebased onto post-behavior state. Preconditions a fresh worker must take as given:

- **`ElementType::is_view()` exists** and is the single node-pool filter in `parse.rs::build_model` and `validate.rs::link` (it replaced both `!= Diagram && !matches!(Behavior(_))` filters). Do NOT re-add the old filter and do NOT revert `is_view()`. This slice does not touch that predicate or `validate.rs` at all.
- **The flow types are already reshaped in `model.rs`:** `FlowNode` is now `ActivityNode`, `FlowEdgeKind` exists, `FlowEdge`/`FlowDoc` are reshaped, and `Model` carries `activity_nodes: Vec<ActivityNode>` + `flow_edges: Vec<FlowEdge>` pools (wire `activityNodes` / `flowEdges`). `FlowDoc.nodes` / `.edges` are `Vec<String>` pool keys. Leave ALL of this intact — this slice reshapes only the sequence/interaction types and adds nothing to the shared `Model.nodes` / `Model.edges` structural pools.
- **The `parse.rs` `use crate::model::{…}` block already imports** `ActivityNode`, `FlowEdge`, `FlowEdgeKind` (behavior renamed `FlowNode`→`ActivityNode` and added `FlowEdgeKind` there). This slice swaps ONLY the sequence names (`Lifeline`/`SeqItem`/`SeqOperand` → `SeqChild`/`SeqEdge`/`SeqNode`) inside that same block, leaving the flow names untouched — see the corrected Step 1.4.
- **`ModelGraph` / `toModelGraph` (TS) already carry** `activityNodes?` / `flowEdges?` pools and a key-based `flows?` view; the flow re-export regions of `packages/wasm/src/index.ts` and `packages/okf/src/types.ts` already read `ActivityNode` / `FlowEdgeKind`. This slice adds/reshapes only the sequence region additively — no renames of, or collisions with, the behavior pools.
- **All quoted line numbers below predate the behavior slice.** The behavior split GROWS the flow-types section of `model.rs` and the `build_flows` function of `parse.rs` (both sit *above* this slice's edit sites), so every absolute line reference here is offset downward after behavior landed. Locate each edit by struct/enum/function NAME (given at every step), not by the stale line number. The edit sites themselves are disjoint from the behavior slice's, so the diffs apply cleanly on top.

## Global Constraints

- **Full CI gate, in this order** (from `.github/workflows/ci.yml`): `cargo test --workspace` → `pnpm build:wasm` → `pnpm lint` → `pnpm build` → `pnpm -r test`. All must pass before the commit.
- **Storage format is NOT redesigned** (spec §9 non-goal). Do NOT touch the sequence *syntax* layer or round-trip: `crates/waml/src/syntax.rs` (`SeqItemSyntax`, `SeqOperandSyntax`, `LifelineLine`, `MessagesBlock`, `ParsedMessage`), `crates/waml/src/grammar.rs` (`parse_messages_block`/`render_messages_block`/`parse_lifeline_line`/`render_lifeline_line`), and `crates/waml/src/serialize.rs`. Source `.md` bytes round-trip identically before and after this change.
- **Message stays interaction-local and ordered** (spec §6): `SeqEdge` lives inside `SequenceDoc`, never in `Model.edges`; it is NOT a reusable pool edge and NOT an `Association`. Document order = time order — `edges` is emitted in document (pre-order) order and each container's ordered `items` fixes local time order.
- **`Lifeline.ref_` semantics are unchanged** (spec §6): it references a pool classifier (types-by). Do NOT widen the target to `InstanceSpecification` — that is slice §7.4, out of scope.
- **Do NOT touch the behavior/flow pools or view** — `FlowDoc`, `ActivityNode`, `FlowEdge`, `FlowEdgeKind`, the `Model.activity_nodes` / `Model.flow_edges` pools, or `ElementType::is_view()`. The behavior model/view split slice has ALREADY landed (serial predecessor). Do NOT revert `FlowNode`→`ActivityNode`, do NOT drop `FlowEdgeKind`, and do NOT re-add the old `!= Diagram && !matches!(Behavior(_))` node-pool filter.
- **Do NOT edit files under `docs/`.**
- **Do NOT hand-edit generated/auto files:** `packages/wasm/src/generated/waml_wasm.d.ts` (+ `.js`) is regenerated by `pnpm build:wasm`; `packages/core/src/templates/orders-checkout-sequence.bundle.ts` is `waml bundle` output (its markdown is unchanged by this slice anyway).
- **Idiomatic Rust:** `cargo fmt` touched files and introduce no new `cargo clippy` warnings before the commit. Exhaustive matches over the `SeqNode`/`SeqChild` variants — no `_ =>` catch-alls.

---

## File Structure

This slice is a single atomic vertical: the Rust runtime shape, the regenerated wasm binding it produces, and the TS consumers that read it are coupled through the generated `waml_wasm.d.ts`, so they land in one green commit. Files touched:

**Rust (runtime model + builder + tests):**
- `crates/waml/src/model.rs` — replace `Lifeline`, `SeqOperand`, `SeqItem`, and reshape `SequenceDoc` with `SeqChild`, `SeqEdge`, `SeqNode`, `SequenceDoc`. `MessageVerb` and `FragmentKind` are unchanged. (Pre-behavior these sat at ~`Lifeline` 508-525 / `SeqOperand` 527-539 / `SeqItem` 541-562 / `SequenceDoc` 564-579, with `MessageVerb` 441-475 / `FragmentKind` 477-505; the behavior slice grew the flow-types section above them, so all of these are offset downward — locate by name.)
- `crates/waml/src/parse.rs` — the `use crate::model::{…}` import block; rewrite `build_interactions`; rewrite the `builds_sequence_doc_with_resolved_lifelines_and_nested_messages` test. (Pre-behavior: import block 12-15, `build_interactions` 778-849, test 1427-1453; behavior's `build_flows` rewrite above `build_interactions` shifts these downward — locate by name.)
- `crates/waml/tests/serde_shape.rs` — rewrite the `sequence_doc_json_matches_ts_field_names` test (145-163) to the new wire shape.

**Regenerated binding (do not hand-edit):**
- `packages/wasm/src/generated/waml_wasm.d.ts` (+ `.js`) — regenerated by `pnpm build:wasm`.

**TS type barrels (mirror the regenerated names):**
- `packages/wasm/src/index.ts` — re-export list (46-51).
- `packages/okf/src/types.ts` — re-export list (18-30).

**TS consumers (rebuild from the flat shape):**
- `packages/web/src/canvas/sequenceLayout.ts` — full rewrite of `layoutSequence` + `SeqRow`.
- `packages/web/src/components/canvas/sequence/SequenceView.svelte` — read `row.edge.*` instead of `row.item.*`.
- `packages/web/src/canvas/sequenceLayout.test.ts` — flat fixtures.
- `packages/web/src/components/canvas/sequence/SequenceView.test.ts` — flat fixtures.
- `packages/core/src/state/overlay.test.ts` — flat seq fixture (195-203).
- `packages/core/src/state/diagrams.test.ts` — flat empty fixtures (36, 50).
- `packages/core/src/nav/tree.test.ts` — flat empty fixture (31).

**Verified NO change** (read only `.key`/`.title`, or pass through opaquely): `crates/waml/src/validate.rs` (walks the syntax layer, not the model), `crates/waml-editor/src/tree.rs`, `packages/core/src/state/overlay.ts`, `packages/core/src/state/diagrams.ts`, `packages/core/src/nav/tree.ts`, `packages/core/src/nav/search.ts`, `packages/core/src/templates/*`.

No files are created or deleted.

---

## Task 1: Flatten the sequence runtime model end-to-end

One atomic deliverable: replace the nested runtime sequence model with the flat nodes+edges shape, regenerate the binding, and rebuild every TS consumer. The generated `waml_wasm.d.ts` couples Rust and TS, so a green CI gate requires all of it in one commit (a Rust-only intermediate would drift the checked-in binding and fail `pnpm build`).

**Files:** as listed in File Structure.

**Interfaces:**
- Produces (Rust, `crate::model`):
  - `pub enum SeqChild { Message { edge: String }, Fragment { node: String } }` (serde tag `item`, lowercase).
  - `pub struct SeqEdge { pub id: String, pub from: String, pub verb: MessageVerb, pub to: String, pub signature: Option<String> }`.
  - `pub enum SeqNode { Lifeline { id, title, alias, ref_ }, Fragment { id, kind: FragmentKind, operands: Vec<String> }, Operand { id, guard: Option<String>, items: Vec<SeqChild> } }` (serde tag `node`, lowercase; `ref_` renamed `ref`).
  - `pub struct SequenceDoc { pub key, pub title, pub describes: Option<String>, pub nodes: Vec<SeqNode>, pub edges: Vec<SeqEdge>, pub items: Vec<SeqChild> }`.
- Produces (TS, `@waml/okf` / `@waml/wasm`): `SeqChild = { item: "message"; edge: string } | { item: "fragment"; node: string }`; `SeqEdge { id; from; verb: MessageVerb; to; signature? }`; `SeqNode = { node: "lifeline"; id; title; alias?; ref? } | { node: "fragment"; id; kind: FragmentKind; operands: string[] } | { node: "operand"; id; guard?; items: SeqChild[] }`; `SequenceDoc { key; title; describes?; nodes: SeqNode[]; edges: SeqEdge[]; items: SeqChild[] }`.
- Consumes: existing `MessageVerb`, `FragmentKind` (unchanged); `Model.interactions: Vec<SequenceDoc>` (field name unchanged); the syntax layer `SeqItemSyntax`/`MessagesBlock`/`LifelineLine` (read-only).

### Phase A — Rust runtime model (TDD)

- [ ] **Step 1.1: Write the failing wire-shape test.** In `crates/waml/tests/serde_shape.rs`, replace the whole `sequence_doc_json_matches_ts_field_names` test (currently lines 145-163) with the new-shape version:

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
    // Lifelines are tagged nodes keyed by their handle; `ref`/`alias` preserved.
    assert_eq!(s["nodes"][0]["node"], "lifeline");
    assert_eq!(s["nodes"][0]["id"], "order");
    assert_eq!(s["nodes"][0]["ref"], "s/order");
    assert_eq!(s["nodes"][0]["alias"], "order");
    // Messages become ordered edges (`m0`, `m1`, … in time order).
    assert_eq!(s["edges"][0]["id"], "m0");
    assert_eq!(s["edges"][0]["verb"], "calls");
    assert_eq!(s["edges"][0]["signature"], "tick()");
    // The root item stream references the edge, then the fragment (document order).
    assert_eq!(s["items"][0]["item"], "message");
    assert_eq!(s["items"][0]["edge"], "m0");
    assert_eq!(s["items"][1]["item"], "fragment");
    assert_eq!(s["items"][1]["node"], "f0");
    // Containment: the operand is emitted before its fragment; guard + nested edge kept.
    assert_eq!(s["nodes"][1]["node"], "operand");
    assert_eq!(s["nodes"][1]["id"], "f0.o0");
    assert_eq!(s["nodes"][1]["guard"], "ready");
    assert_eq!(s["nodes"][1]["items"][0]["edge"], "m1");
    assert_eq!(s["nodes"][2]["node"], "fragment");
    assert_eq!(s["nodes"][2]["kind"], "opt");
    assert_eq!(s["nodes"][2]["operands"][0], "f0.o0");
}
```

- [ ] **Step 1.2: Run it, verify it fails to compile.** Run:
  ```
  cargo test -p waml --features serde --test serde_shape sequence_doc_json_matches_ts_field_names
  ```
  Expected: FAIL to compile / assertion failure — the emitted JSON still has `lifelines`/`messages`, so `s["nodes"]`/`s["edges"]`/`s["items"]` are `null` and the string asserts fail (the model shape does not exist yet).

- [ ] **Step 1.3: Replace the sequence model types in `model.rs`.** In `crates/waml/src/model.rs`, replace the four items `Lifeline`, `SeqOperand`, `SeqItem`, and `SequenceDoc` — i.e. the whole span from the `/// A sequence participant…` doc comment down to the end of the `SequenceDoc` struct — with the block below. (Locate the span by that doc comment; the pre-behavior line numbers `Lifeline` 508-525 / `SeqOperand` 527-539 / `SeqItem` 541-562 / `SequenceDoc` 564-579 are offset downward now that the behavior slice grew the flow-types section above. `MessageVerb`/`FragmentKind` sit just above this span and stay unchanged.)

```rust
/// A message reference or a nested-fragment reference inside an ordered
/// interaction stream (the interaction root, or a fragment operand). Document
/// order within the list is time order (design spec §6). `edge`/`node` are ids
/// into `SequenceDoc.edges` / `SequenceDoc.nodes`.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(tag = "item", rename_all = "lowercase"))]
#[cfg_attr(feature = "wasm", derive(tsify_next::Tsify))]
#[cfg_attr(feature = "wasm", tsify(into_wasm_abi, from_wasm_abi))]
pub enum SeqChild {
    Message { edge: String },
    Fragment { node: String },
}

/// A message: an interaction-LOCAL, ORDERED edge (design spec §6). It is NOT a
/// reusable pool edge and NOT an Association — its identity is bound to this
/// interaction's time axis. `from`/`to` are lifeline node ids (a lifeline's
/// handle: its alias, else its title).
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "wasm", derive(tsify_next::Tsify))]
#[cfg_attr(feature = "wasm", tsify(into_wasm_abi, from_wasm_abi))]
pub struct SeqEdge {
    /// Doc-unique id (`m0`, `m1`, … in document/time order), referenced by a
    /// container's ordered `items`.
    pub id: String,
    pub from: String,
    pub verb: MessageVerb,
    pub to: String,
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub signature: Option<String>,
}

/// A node of an interaction's flat model: a participant lifeline, a combined
/// fragment, or a fragment operand. These are interaction-LOCAL (design spec
/// §6) — not members of the shared Element pool. Containment is preserved by id
/// reference: a fragment lists its operand ids; an operand lists its ordered
/// items (message edges + nested fragment nodes).
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(tag = "node", rename_all = "lowercase"))]
#[cfg_attr(feature = "wasm", derive(tsify_next::Tsify))]
#[cfg_attr(feature = "wasm", tsify(into_wasm_abi, from_wasm_abi))]
pub enum SeqNode {
    /// A participant column. `ref_` types-by a pool CLASSIFIER (design spec §6);
    /// widening to `InstanceSpecification` is §7.4 (out of scope here). `id` is
    /// the lifeline handle (alias, else title) that messages reference.
    Lifeline {
        id: String,
        title: String,
        #[cfg_attr(
            feature = "serde",
            serde(default, skip_serializing_if = "Option::is_none")
        )]
        alias: Option<String>,
        #[cfg_attr(
            feature = "serde",
            serde(rename = "ref", default, skip_serializing_if = "Option::is_none")
        )]
        ref_: Option<String>,
    },
    /// A combined fragment (`alt`/`opt`/`loop`). `operands` are its `Operand`
    /// node ids, in order.
    Fragment {
        id: String,
        kind: FragmentKind,
        operands: Vec<String>,
    },
    /// One operand of a combined fragment. `guard: None` = the `else` operand.
    /// `items` is the ordered message/fragment stream (time order).
    Operand {
        id: String,
        #[cfg_attr(
            feature = "serde",
            serde(default, skip_serializing_if = "Option::is_none")
        )]
        guard: Option<String>,
        items: Vec<SeqChild>,
    },
}

/// One interaction (`uml.Sequence`): a flat, interaction-local model of
/// lifelines/fragments/operands (`nodes`) and ordered messages (`edges`), with
/// containment preserved via `items` (the root stream) plus each fragment's
/// operand ids and each operand's item stream. This is the RUNTIME view; the
/// on-disk `## Lifelines`/`## Messages` form (nested) is a separate storage
/// shape (design spec §9 — storage/runtime need not be 1:1).
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "wasm", derive(tsify_next::Tsify))]
#[cfg_attr(feature = "wasm", tsify(into_wasm_abi, from_wasm_abi))]
pub struct SequenceDoc {
    pub key: String,
    pub title: String,
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub describes: Option<String>,
    /// Lifelines + fragments + operands; resolve by `id`. Lifelines appear first,
    /// in declaration order (participant column order).
    pub nodes: Vec<SeqNode>,
    /// Messages, ORDERED (document order = time order); interaction-local.
    pub edges: Vec<SeqEdge>,
    /// The interaction root's ordered item stream (message/fragment refs).
    pub items: Vec<SeqChild>,
}
```

- [ ] **Step 1.4: Update the `parse.rs` import list.** In `crates/waml/src/parse.rs`, change the `use crate::model::{…}` block — replace ONLY the sequence type names `Lifeline`, `SeqItem`, `SeqOperand` with `SeqChild`, `SeqEdge`, `SeqNode` (keep `SequenceDoc`; leave the behavior slice's `ActivityNode` / `FlowEdge` / `FlowEdgeKind` and every other name untouched). After the behavior slice landed the block reads (before-state — note `ActivityNode` / `FlowEdgeKind`, NO `FlowNode`):

```rust
use crate::model::{
    ActivityNode, Attribute, BehaviorKind, Diagram, DiagramDisplay, DiagramGroup, Edge, ElementType,
    FlowDoc, FlowEdge, FlowEdgeKind, FlowFlavor, Lifeline, Model, Node, SeqItem, SeqOperand,
    SequenceDoc,
};
```

  Rewrite it to (after-state — sequence names swapped, flow names preserved):

```rust
use crate::model::{
    ActivityNode, Attribute, BehaviorKind, Diagram, DiagramDisplay, DiagramGroup, Edge, ElementType,
    FlowDoc, FlowEdge, FlowEdgeKind, FlowFlavor, Model, Node, SeqChild, SeqEdge, SeqNode,
    SequenceDoc,
};
```

- [ ] **Step 1.5: Rewrite `build_interactions` to emit the flat model.** In `crates/waml/src/parse.rs`, replace the entire `fn build_interactions(…) { … }` body (778-849) with:

```rust
fn build_interactions(parsed: &[ParsedDoc], keyset: &HashSet<&str>) -> Vec<SequenceDoc> {
    use crate::syntax::SeqItemSyntax;

    // Walk the nested `## Messages` syntax into the flat pools, assigning
    // deterministic ids: `m{n}` messages in document (time) order, `f{n}`
    // fragments in pre-order, `f{n}.o{j}` operands. Recursive.
    fn build_items(
        src: &[Line<SeqItemSyntax>],
        handle_of: &dyn Fn(&str) -> String,
        nodes: &mut Vec<SeqNode>,
        edges: &mut Vec<SeqEdge>,
        msg: &mut usize,
        frag: &mut usize,
    ) -> Vec<SeqChild> {
        let mut out = Vec::new();
        for it in src.iter().filter_map(Line::parsed) {
            match it {
                SeqItemSyntax::Message(m) => {
                    let id = format!("m{}", *msg);
                    *msg += 1;
                    edges.push(SeqEdge {
                        id: id.clone(),
                        from: handle_of(&m.from),
                        verb: m.verb,
                        to: handle_of(&m.to),
                        signature: m.signature.clone(),
                    });
                    out.push(SeqChild::Message { edge: id });
                }
                SeqItemSyntax::Fragment { kind, operands, .. } => {
                    let fid = format!("f{}", *frag);
                    *frag += 1;
                    let mut operand_ids = Vec::new();
                    for (j, o) in operands.iter().enumerate() {
                        let oid = format!("{fid}.o{j}");
                        let items = build_items(&o.items, handle_of, nodes, edges, msg, frag);
                        nodes.push(SeqNode::Operand {
                            id: oid.clone(),
                            guard: o.guard.clone(),
                            items,
                        });
                        operand_ids.push(oid);
                    }
                    nodes.push(SeqNode::Fragment {
                        id: fid.clone(),
                        kind: *kind,
                        operands: operand_ids,
                    });
                    out.push(SeqChild::Fragment { node: fid });
                }
            }
        }
        out
    }

    let mut out = Vec::new();
    for p in parsed {
        if p.ty != ElementType::Behavior(BehaviorKind::Sequence) {
            continue;
        }
        // Lifelines become `SeqNode::Lifeline`s (participant columns), in
        // declaration order. A lifeline's id is its handle: alias, else title.
        let mut lifelines: Vec<SeqNode> = Vec::new();
        for s in &p.doc.sections {
            let Section::Lifelines(lines) = s else { continue };
            for l in lines.iter().filter_map(Line::parsed) {
                let resolved = crate::okf::resolve_href(&p.path, &l.link.slug);
                let alias = l.alias.clone();
                let title = l.link.title.clone();
                let id = alias.clone().unwrap_or_else(|| title.clone());
                lifelines.push(SeqNode::Lifeline {
                    id,
                    title,
                    alias,
                    ref_: keyset.contains(resolved.as_str()).then_some(resolved),
                });
            }
        }
        // A participant token (alias, title, or `[Title](./slug.md)` link)
        // canonicalizes to a lifeline's handle (its id). Unresolved tokens are
        // kept verbatim (validate warns; render degrades). Scoped so its borrow
        // of `lifelines` ends before we move `lifelines` into `nodes`.
        let (frag_nodes, edges, items) = {
            let handle_of = |token: &str| -> String {
                let name = match crate::grammar::parse_link_ref(token) {
                    Some(l) => l.title,
                    None => token.to_string(),
                };
                for n in &lifelines {
                    if let SeqNode::Lifeline { id, title, alias, .. } = n {
                        if alias.as_deref() == Some(name.as_str()) || *title == name {
                            return id.clone();
                        }
                    }
                }
                name
            };
            let mut frag_nodes: Vec<SeqNode> = Vec::new();
            let mut edges: Vec<SeqEdge> = Vec::new();
            let mut msg = 0usize;
            let mut frag = 0usize;
            let mut items: Vec<SeqChild> = Vec::new();
            for s in &p.doc.sections {
                if let Section::Messages(block) = s {
                    items = build_items(&block.items, &handle_of, &mut frag_nodes, &mut edges, &mut msg, &mut frag);
                }
            }
            (frag_nodes, edges, items)
        };
        let mut nodes = lifelines;
        nodes.extend(frag_nodes);
        out.push(SequenceDoc {
            key: p.id.clone(),
            title: doc_title(p),
            describes: resolve_describes(p, keyset),
            nodes,
            edges,
            items,
        });
    }
    out
}
```

- [ ] **Step 1.6: Rewrite the `build_interactions` unit test.** In `crates/waml/src/parse.rs`, replace the whole `builds_sequence_doc_with_resolved_lifelines_and_nested_messages` test (1427-1453) with:

```rust
    #[test]
    fn builds_sequence_doc_with_resolved_lifelines_and_nested_messages() {
        use crate::model::{FragmentKind, MessageVerb, SeqChild, SeqNode};
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

        // Node lookup by id (works across every variant via an or-pattern).
        let by_id = |id: &str| {
            s.nodes
                .iter()
                .find(|n| match n {
                    SeqNode::Lifeline { id: i, .. }
                    | SeqNode::Fragment { id: i, .. }
                    | SeqNode::Operand { id: i, .. } => i == id,
                })
                .unwrap()
        };

        // Lifelines are `SeqNode::Lifeline`s, first and in declaration order.
        let lifelines: Vec<_> = s.nodes.iter().filter(|n| matches!(n, SeqNode::Lifeline { .. })).collect();
        assert_eq!(lifelines.len(), 3);
        let SeqNode::Lifeline { id, ref_, .. } = lifelines[0] else { panic!() };
        assert_eq!(id, "Customer");
        assert_eq!(ref_.as_deref(), Some("s/customer"));
        let SeqNode::Lifeline { alias, .. } = lifelines[1] else { panic!() };
        assert_eq!(alias.as_deref(), Some("order"));
        let SeqNode::Lifeline { ref_, .. } = lifelines[2] else { panic!() };
        assert_eq!(*ref_, None, "unresolved lifeline degrades to link title only");

        // Four messages become four ordered edges (document/time order).
        assert_eq!(s.edges.len(), 4);

        // Root item stream: message, fragment, message.
        assert_eq!(s.items.len(), 3);
        let SeqChild::Message { edge } = &s.items[0] else { panic!() };
        let m0 = s.edges.iter().find(|e| &e.id == edge).unwrap();
        assert_eq!((m0.from.as_str(), m0.verb, m0.to.as_str()), ("Customer", MessageVerb::Calls, "order"));
        assert_eq!(m0.signature.as_deref(), Some("place(items)"));
        assert!(matches!(&s.items[2], SeqChild::Message { .. }));

        // The fragment keeps its kind + two operands; the first operand keeps its guard.
        let SeqChild::Fragment { node } = &s.items[1] else { panic!() };
        let SeqNode::Fragment { kind, operands, .. } = by_id(node) else { panic!() };
        assert_eq!(*kind, FragmentKind::Alt);
        assert_eq!(operands.len(), 2);
        let SeqNode::Operand { guard, items, .. } = by_id(&operands[0]) else { panic!() };
        assert_eq!(guard.as_deref(), Some("paid"));
        assert_eq!(items.len(), 1, "the `paid` operand carries the `ship()` message");
    }
```

- [ ] **Step 1.7: Run the Rust tests, verify green.** Run:
  ```
  cargo test -p waml
  ```
  Expected: PASS, including `sequence_doc_json_matches_ts_field_names`, `builds_sequence_doc_with_resolved_lifelines_and_nested_messages`, and the existing `message_verbs_and_fragment_kinds_round_trip` (unchanged — `MessageVerb`/`FragmentKind` are untouched).

- [ ] **Step 1.8: Run the whole workspace + clippy.** Run:
  ```
  cargo test --workspace
  cargo clippy -p waml --all-targets
  cargo fmt
  ```
  Expected: `cargo test --workspace` all green (round-trip/serialize/validate tests unchanged — storage layer untouched); `cargo clippy` reports no new warnings; `cargo fmt` formats the two touched Rust source files.

### Phase B — Regenerate the wasm binding

- [ ] **Step 1.9: Regenerate the wasm bindings.** Run:
  ```
  pnpm build:wasm
  ```
  Then confirm the generated types changed as expected:
  ```
  git diff packages/wasm/src/generated/waml_wasm.d.ts
  ```
  Expected: `Lifeline`, `SeqOperand`, and the `SeqItem` union are gone; new exports appear:
  ```ts
  export type SeqChild = { item: "message"; edge: string } | { item: "fragment"; node: string };
  export interface SeqEdge { id: string; from: string; verb: MessageVerb; to: string; signature?: string }
  export type SeqNode =
      | { node: "lifeline"; id: string; title: string; alias?: string; ref?: string }
      | { node: "fragment"; id: string; kind: FragmentKind; operands: string[] }
      | { node: "operand"; id: string; guard?: string; items: SeqChild[] };
  export interface SequenceDoc { key: string; title: string; describes?: string; nodes: SeqNode[]; edges: SeqEdge[]; items: SeqChild[] }
  ```
  Do NOT hand-edit this file — if the shape is wrong, fix the Rust in Phase A and rerun.

### Phase C — TS type barrels

- [ ] **Step 1.10: Update the `@waml/wasm` re-export barrel.** In `packages/wasm/src/index.ts`, in the `export type { … } from "./generated/waml_wasm.js";` block, replace the three lines `Lifeline,` / `SeqItem,` / `SeqOperand,` (pre-behavior lines 47-49, now shifted — the behavior slice already swapped `FlowNode,`→`ActivityNode,` and added `FlowEdgeKind,` in the flow region above; leave those intact) with the new names, keeping `SequenceDoc` and the surrounding entries:

```ts
  SequenceDoc,
  SeqNode,
  SeqEdge,
  SeqChild,
  MessageVerb,
  FragmentKind,
```

- [ ] **Step 1.11: Update the `@waml/okf` re-export barrel.** In `packages/okf/src/types.ts`, in the `export type { … } from "@waml/wasm";` block, replace the three lines `Lifeline,` / `SeqOperand,` / `SeqItem,` (pre-behavior lines 20-22, now shifted — the behavior slice already swapped `FlowNode,`→`ActivityNode,` and added `FlowEdgeKind,` in the flow region; leave those intact) so the sequence names read:

```ts
  MessageVerb,
  FragmentKind,
  SeqEdge,
  SeqNode,
  SeqChild,
  SequenceDoc,
```
  (Leave the `import type { … }` block below it unchanged — it already imports only `SequenceDoc` among the sequence types.)

### Phase D — TS consumers

- [ ] **Step 1.12: Rewrite `sequenceLayout.ts` for the flat model.** Replace the entire contents of `packages/web/src/canvas/sequenceLayout.ts` with:

```ts
import type { SeqChild, SeqEdge, SeqNode, SequenceDoc } from "@waml/okf";

// ── Sequence substrate layout (behavioral substrates spec) ───────────────────
// Purely deterministic: document order fixes row Y, lifeline declaration order
// fixes column X. No constraint solving — the flat interaction model (lifeline
// nodes, message edges, and the ordered `items` stream) IS the layout.

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
  | { kind: "message"; y: number; fromX: number; toX: number; edge: SeqEdge; self: boolean }
  | { kind: "fragmentStart"; y: number; depth: number; label: string; x0: number; x1: number }
  | { kind: "fragmentEnd"; y: number; depth: number; x0: number; x1: number }
  | { kind: "operandDivider"; y: number; depth: number; label?: string; x0: number; x1: number };

export interface SequenceLayout {
  lifelines: LaneLayout[];
  rows: SeqRow[];
  width: number;
  height: number;
}

export function layoutSequence(doc: SequenceDoc): SequenceLayout {
  const edgeById = new Map<string, SeqEdge>(doc.edges.map((e): [string, SeqEdge] => [e.id, e]));
  const nodeById = new Map<string, SeqNode>(doc.nodes.map((n): [string, SeqNode] => [n.id, n]));

  const lifelines: LaneLayout[] = doc.nodes
    .filter((n): n is Extract<SeqNode, { node: "lifeline" }> => n.node === "lifeline")
    .map((l, i) => ({ key: l.ref ?? l.id, handle: l.id, x: LANE_MARGIN + i * LANE_WIDTH }));
  const xOf = (id: string): number => lifelines.find((l) => l.handle === id)?.x ?? LANE_MARGIN;

  // A fragment spans every lane touched by messages inside it (min..max),
  // padded so its frame clears the endpoints.
  const bounds = (items: SeqChild[]): [number, number] => {
    let lo = Infinity;
    let hi = -Infinity;
    for (const c of items) {
      if (c.item === "message") {
        const e = edgeById.get(c.edge);
        if (!e) continue;
        lo = Math.min(lo, xOf(e.from), xOf(e.to));
        hi = Math.max(hi, xOf(e.from), xOf(e.to));
      } else {
        const frag = nodeById.get(c.node);
        if (!frag || frag.node !== "fragment") continue;
        for (const oid of frag.operands) {
          const op = nodeById.get(oid);
          if (!op || op.node !== "operand") continue;
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

  const walk = (items: SeqChild[], depth: number): void => {
    for (const c of items) {
      if (c.item === "message") {
        const e = edgeById.get(c.edge);
        if (!e) continue;
        const fromX = xOf(e.from);
        const toX = xOf(e.to);
        rows.push({ kind: "message", y, fromX, toX, edge: e, self: fromX === toX });
        y += ROW_HEIGHT;
      } else {
        const frag = nodeById.get(c.node);
        if (!frag || frag.node !== "fragment") continue;
        const [lo, hi] = bounds([c]);
        const x0 = lo - 30 - depth * 12;
        const x1 = hi + 30 + depth * 12;
        rows.push({ kind: "fragmentStart", y, depth, label: frag.kind, x0, x1 });
        y += FRAGMENT_HEADER_HEIGHT;
        frag.operands.forEach((oid, i) => {
          const op = nodeById.get(oid);
          if (!op || op.node !== "operand") return;
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
  walk(doc.items, 0);

  const width = lifelines.length > 0 ? Math.max(...lifelines.map((l) => l.x)) + LANE_MARGIN : LANE_MARGIN * 2;
  return { lifelines, rows, width, height: y + 40 };
}
```

- [ ] **Step 1.13: Update `SequenceView.svelte` to read `row.edge`.** In `packages/web/src/components/canvas/sequence/SequenceView.svelte`, change the three verb predicates and the signature reference from `r.item.*` / `row.item.*` to `r.edge.*` / `row.edge.*`. Replace lines 12-14:

```svelte
  const dashed = (r: MessageRow) => r.edge.verb === "replies" || r.edge.verb === "creates";
  const filled = (r: MessageRow) => r.edge.verb === "calls";
  const destroyed = (r: MessageRow) => r.edge.verb === "destroys";
```
  and replace the signature guard/text (line 48-49) inside the `{#if row.kind === "message"}` block:

```svelte
        {#if row.edge.signature}
          <text x={(row.fromX + row.toX) / 2} y={row.y - 6} text-anchor="middle" font-size="11" fill="#334155">{row.edge.signature}</text>
        {/if}
```
  (Everything else in the file — the `MessageRow` alias, `markerFor`, the SVG markers, lifeline rendering — is unchanged.)

- [ ] **Step 1.14: Rewrite the `sequenceLayout.test.ts` fixture.** Replace the entire contents of `packages/web/src/canvas/sequenceLayout.test.ts` with:

```ts
import { describe, expect, it } from "vitest";
import type { SequenceDoc } from "@waml/okf";
import { layoutSequence } from "./sequenceLayout";

const DOC: SequenceDoc = {
  key: "s/place-order",
  title: "Place Order",
  nodes: [
    { node: "lifeline", id: "Customer", title: "Customer", ref: "s/customer" },
    { node: "lifeline", id: "order", title: "Order", alias: "order", ref: "s/order" },
    { node: "lifeline", id: "wh", title: "Warehouse", alias: "wh" },
    { node: "operand", id: "f0.o0", guard: "paid", items: [{ item: "message", edge: "m1" }] },
    { node: "operand", id: "f0.o1", items: [{ item: "message", edge: "m2" }] },
    { node: "fragment", id: "f0", kind: "alt", operands: ["f0.o0", "f0.o1"] },
  ],
  edges: [
    { id: "m0", from: "Customer", verb: "calls", to: "order", signature: "place(items)" },
    { id: "m1", from: "order", verb: "calls", to: "wh", signature: "ship()" },
    { id: "m2", from: "order", verb: "sends", to: "Customer", signature: "paymentFailed()" },
    { id: "m3", from: "order", verb: "replies", to: "Customer" },
  ],
  items: [
    { item: "message", edge: "m0" },
    { item: "fragment", node: "f0" },
    { item: "message", edge: "m3" },
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
      edges: [{ id: "m0", from: "order", verb: "calls", to: "order", signature: "validate()" }],
      items: [{ item: "message", edge: "m0" }],
    };
    const l = layoutSequence(selfDoc);
    const row = l.rows[0] as Extract<(typeof l.rows)[number], { kind: "message" }>;
    expect(row.self).toBe(true);
  });
});
```

- [ ] **Step 1.15: Rewrite the `SequenceView.test.ts` fixtures.** Replace the two `SequenceDoc` fixtures in `packages/web/src/components/canvas/sequence/SequenceView.test.ts` with the flat shape (assertions unchanged). Replace the `const DOC` block (lines 6-14):

```ts
const DOC: SequenceDoc = {
  key: "s/place-order",
  title: "Place Order",
  nodes: [
    { node: "lifeline", id: "Customer", title: "Customer" },
    { node: "lifeline", id: "order", title: "Order", alias: "order" },
  ],
  edges: [
    { id: "m0", from: "Customer", verb: "calls", to: "order", signature: "place(items)" },
    { id: "m1", from: "order", verb: "replies", to: "Customer", signature: "confirmation" },
  ],
  items: [
    { item: "message", edge: "m0" },
    { item: "message", edge: "m1" },
  ],
};
```
  and the inner `const doc` block in the destroys test (lines 26-34):

```ts
    const doc: SequenceDoc = {
      key: "s/teardown",
      title: "Teardown",
      nodes: [
        { node: "lifeline", id: "Manager", title: "Manager" },
        { node: "lifeline", id: "Worker", title: "Worker" },
      ],
      edges: [
        { id: "m0", from: "Manager", verb: "sends", to: "Worker", signature: "stop()" },
        { id: "m1", from: "Manager", verb: "destroys", to: "Worker" },
      ],
      items: [
        { item: "message", edge: "m0" },
        { item: "message", edge: "m1" },
      ],
    };
```

- [ ] **Step 1.16: Update the `overlay.test.ts` seq fixture.** In `packages/core/src/state/overlay.test.ts`, replace the `const seq: SequenceDoc = { … }` object (lines 195-203) with the flat shape (the test only asserts pass-through equality, so any valid instance works):

```ts
    const seq: SequenceDoc = {
      key: "s/place-order",
      title: "Place Order",
      nodes: [
        { node: "lifeline", id: "Customer", title: "Customer", ref: "s/customer" },
        { node: "lifeline", id: "order", title: "Order", alias: "order" },
        { node: "operand", id: "f0.o0", guard: "paid", items: [] },
        { node: "operand", id: "f0.o1", items: [] },
        { node: "fragment", id: "f0", kind: "alt", operands: ["f0.o0", "f0.o1"] },
      ],
      edges: [{ id: "m0", from: "Customer", verb: "calls", to: "order", signature: "place(items)" }],
      items: [
        { item: "message", edge: "m0" },
        { item: "fragment", node: "f0" },
      ],
    };
```

- [ ] **Step 1.17: Update the `diagrams.test.ts` empty fixtures.** In `packages/core/src/state/diagrams.test.ts`, change both occurrences of the interaction stub `{ key: "s1", title: "S", lifelines: [], messages: [] }` (lines 36 and 50) to:
  ```ts
      interactions: [{ key: "s1", title: "S", nodes: [], edges: [], items: [] }] as ModelGraph["interactions"],
  ```

- [ ] **Step 1.18: Update the `tree.test.ts` empty fixture.** In `packages/core/src/nav/tree.test.ts`, change the interaction stub on line 31 to:
  ```ts
  interactions: [{ key: "checkout-seq", title: "Checkout sequence", nodes: [], edges: [], items: [] }],
  ```

### Phase E — Full gate + commit

- [ ] **Step 1.19: Run the rest of the gate, verify green.** Run in order:
  ```
  pnpm lint
  pnpm build
  pnpm -r test
  ```
  Expected: all green. `pnpm lint` (tsc + eslint) passes — the `n is Extract<…>` guard and the explicit `[string, SeqNode]` map tuples type-check; `pnpm build` compiles `@waml/okf` dist (needed before web tests resolve the package); `pnpm -r test` passes, including the updated `sequenceLayout`, `SequenceView`, `overlay`, `diagrams`, and `tree` suites.

- [ ] **Step 1.20: Commit.** Run:
  ```
  git add crates/waml/src/model.rs crates/waml/src/parse.rs crates/waml/tests/serde_shape.rs packages/wasm/src/generated/waml_wasm.d.ts packages/wasm/src/generated/waml_wasm.js packages/wasm/src/index.ts packages/okf/src/types.ts packages/web/src/canvas/sequenceLayout.ts packages/web/src/canvas/sequenceLayout.test.ts packages/web/src/components/canvas/sequence/SequenceView.svelte packages/web/src/components/canvas/sequence/SequenceView.test.ts packages/core/src/state/overlay.test.ts packages/core/src/state/diagrams.test.ts packages/core/src/nav/tree.test.ts
  git commit -m "feat(model): flatten SequenceDoc into unified nodes+edges shape"
  ```
  (Add any other `packages/wasm/src/generated/*` files `pnpm build:wasm` regenerated — e.g. `wasm-inline` — as reported by `git status`.)

---

## Notes / risks

- **Why one atomic task:** the checked-in `packages/wasm/src/generated/waml_wasm.d.ts` couples the Rust runtime shape to the TS consumers. A Rust-only intermediate commit would drift that binding (CI runs `pnpm build:wasm`) and fail `pnpm build`, so the whole vertical lands in one green commit. This mirrors the element-pool-rename plan's atomic cross-crate commit.
- **Storage untouched = byte-stable round-trip:** the syntax layer (`SeqItemSyntax`/`MessagesBlock`) and `serialize.rs`/`grammar.rs` are not modified, so `.md` files parse→serialize identically. `validate.rs` walks the syntax layer (not `SequenceDoc`), so it needs no change; its sequence tests stay green.
- **Determinism / stability:** ids are structural (`m{n}` in document/time order, `f{n}` pre-order, `f{n}.o{j}`), so the emitted model is stable for a given source — important because `interactions` serializes into the model JSON.
- **Empty operands are fine:** containment is explicit (`operands` ids + `items` streams), so an `else` operand with no messages still round-trips as `{ node: "operand", id, items: [] }` — no position inference needed.
- **`pnpm build:wasm` needs the wasm toolchain** (`wasm-pack`, `wasm32-unknown-unknown`) — same as every change in this repo and part of the CI gate.

---

## Self-Review

**1. Spec coverage (§6 + task):**
- "lifeline / fragment / operand become nodes" → `SeqNode::{Lifeline, Fragment, Operand}` (Step 1.3). ✓
- "messages become ordered, interaction-local edges" → `SeqEdge` inside `SequenceDoc` (never `Model.edges`), `edges` emitted in document/time order (Steps 1.3, 1.5). ✓
- "containment preserved (fragments contain operands contain items)" → `Fragment.operands` ids + `Operand.items` stream + root `items` (Steps 1.3, 1.5); verified by the wire test and unit test (Steps 1.1, 1.6). ✓
- "Lifeline references a pool element via `ref_`; do NOT widen to instances" → `Lifeline.ref_` kept as-is; doc comment records §7.4 is out of scope (Step 1.3). ✓
- "message is its own edge kind, NOT a pool edge / NOT an Association, ORDERED" → dedicated `SeqEdge`; doc comment + local ordering (Step 1.3). ✓
- Combined fragments `alt`/`opt`/`loop` + guards keep nesting → `FragmentKind` unchanged; `Operand.guard` kept; nesting via ids (Steps 1.3, 1.5). ✓
- §9 non-goal (no storage redesign) → syntax/grammar/serialize untouched (Global Constraints, Notes). ✓

**2. Placeholder scan:** No TBD/TODO/"handle edge cases"/"similar to"/"write tests for the above". Every code step shows complete code. ✓

**3. Type consistency:** `SeqNode`/`SeqEdge`/`SeqChild`/`SequenceDoc` field and variant names are identical across model.rs (1.3), build_interactions (1.5), Rust tests (1.1, 1.6), regenerated binding (1.9), barrels (1.10, 1.11), and TS consumers/fixtures (1.12–1.18): `SeqChild = {item:"message",edge} | {item:"fragment",node}`; `SeqEdge{id,from,verb,to,signature?}`; `SeqNode` tag `node` with `lifeline{id,title,alias?,ref?}`/`fragment{id,kind,operands}`/`operand{id,guard?,items}`; `SequenceDoc{key,title,describes?,nodes,edges,items}`. The frontend reads `row.edge.*` consistently after the `SeqRow` message variant renamed `item`→`edge` (1.12, 1.13). ✓

---

## Cross-slice contract (relative to the already-landed behavior/flow slice)

The behavior slice and this sequence slice **deliberately diverge** on element promotion, and that divergence is spec-sanctioned (design spec §4 vs §6) — not an inconsistency to reconcile away:

- **Flows are pooled; the interaction stays inline — by design.** The behavior slice promotes activity/state-machine elements to model-level pools (`Model.activity_nodes` + typed `Model.flow_edges`) and makes `FlowDoc` a view of pool keys, per spec §4 (reusable activity elements live in the Element pool). This slice keeps `SeqNode`/`SeqEdge`/`SeqChild` **inline on `SequenceDoc`**, per spec §6: a Message is ordered + interaction-local (NOT a reusable pool edge, NOT an Association), and fragments/operands are inherently ordered-local. So flows go pool-level while the interaction stays document-local — the asymmetry is intentional and correct. (An earlier draft of this note wrongly claimed "the flow plan must leave `FlowNode`/`FlowEdge` out of the shared pool"; that was written blind to the behavior slice and is retracted.)
- **Neither slice touches the STRUCTURAL pool or the classifier pool.** Behavior's activity pools (`Model.activity_nodes` / `Model.flow_edges`) are their own model-level pools, separate from the structural `Model.nodes` / `Model.edges`; this slice adds nothing to any shared pool. Both behavior docs and interactions stay OUT of the classifier `Model.nodes` pool via `ElementType::is_view()` (the predicate the behavior slice added and this slice relies on unchanged). Messages stay interaction-local (§6).
- **Type names are substrate-specific and non-colliding:** `ActivityNode` / `FlowEdge` / `FlowEdgeKind` (flow, post-behavior — `FlowNode` was renamed to `ActivityNode`) vs `SeqNode` / `SeqEdge` / `SeqChild` (sequence). Do not introduce a shared `Node`/`Edge` behavior type that would collide with the structural `Model.nodes`/`Model.edges`. Shared "unified shape" vocabulary is only skin-deep: both expose `nodes` + `edges`, and `SequenceDoc` adds an `items` ordered-containment stream that flows lack.
- **`ElementType` / `is_classifier()` / `is_view()` are the shared predicates** both slices rely on unchanged; neither widens `BehaviorKind` or the metaclass set here.
