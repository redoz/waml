# WAML Sequence Language Completeness Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement the approved practical UML sequence language as one canonical pre-1.0 vertical slice from lossless syntax through solver output.

**Architecture:** The Markdown layer first owns `## Gates`. The UML syntax tree then gives every accepted sequence construct a fixed-slot typed node. A focused `uml::sequence` module converts those declared nodes into typed runtime pools, validates references and message correlation, and keeps invalid authored data in the declared layer. The interaction solver uses the typed runtime model in two passes: it assigns rows and frames first, then derives endpoints and activations from stable message identities.

**Tech Stack:** Rust 2021, `waml-syntax` lossless green/red trees, the `waml` UML analysis pipeline, serde feature tests, the existing interaction solver, Cargo, and RTK-prefixed shell commands.

## Global Constraints

- Source order is behavior order outside `par`.
- `par` is the only new construct that removes order between branches.
- The only message spellings are `calls`, `returns`, `signals`, `creates`, and `destroys`.
- `async` is valid only after the target of `calls`.
- The parser must reject `replies`, `sends`, and colon-form calls as messages. It can emit `UnsupportedSequenceForm` for them.
- `outside` is reserved. A lifeline alias cannot be `outside` and cannot contain `@`.
- Do not add a migration mode, a compatibility feature flag, a legacy parser branch, a serde alias, a version adapter, a dual formatter path, or a temporary semantic alias.
- Keep canonical source lossless. Keep malformed source lossless and recover at the next sibling item, operand marker, section heading, or document end.
- Do not add `strict`, `seq`, `ignore`, `consider`, coregions, continuations, general orderings, part decomposition, execution specifications, state invariants, or time constraints.
- Do not change the visual sequence editor. Only make mechanical consumer updates that are necessary after runtime type changes.
- Convert repository sequence documents, fixtures, snapshots, goldens, and serialized expectations in the same implementation branch.
- Preserve the existing unrelated worktree changes in `crates/waml-editor/tests/fixtures/mini/orders-diagram.md` and `docs/waml/architecture/views/domain-model.md`.
- Use ASD-STE100 Simplified Technical English in source comments, diagnostics, tests, and documentation.

---

## Scope and file structure

This is one plan because each layer depends on the previous layer. A parser-only or model-only merge would leave accepted source with missing semantics. The corpus conversion must land with the parser replacement.

### Files to create

- `crates/waml/src/uml/sequence.rs` — own sequence declared-to-runtime lowering, validation, call matching, interaction-use resolution, and cycle detection.
- `crates/waml/tests/sequence_language_syntax.rs` — pin fixed-slot syntax, lossless output, positive grammar cases, and local recovery.
- `crates/waml/tests/sequence_semantics.rs` — pin declared preservation, runtime resolution, validation, call matching, lifetimes, fragments, references, and gates.
- `crates/waml/tests/sequence_formatter.rs` — pin canonical formatting, formatter idempotence, and parse-format-parse declared equality.

### Files to modify

- `crates/waml-syntax/src/markdown/projection.rs` — add `WamlSectionKind::Gates` and recognize `## Gates`.
- `crates/waml-syntax/tests/markdown_extensions.rs` — pin the new section range and order.
- `crates/waml/src/uml/syntax/kind.rs` — add syntax node, slot, token, and diagnostic kinds for gates, endpoint forms, message clauses, branches, refs, and binds.
- `crates/waml/src/uml/syntax/ast.rs` — add typed fixed-slot accessors.
- `crates/waml/src/uml/syntax/parser.rs` — parse canonical sequence messages, fragments, operands, gates, refs, and binds with sibling recovery.
- `crates/waml/src/uml/syntax/mod.rs` — export the new typed nodes and update focused lossless parser tests.
- `crates/waml/src/uml/declared.rs` — replace the legacy declared message shape and add declared endpoint, gate, fragment, operand, binding, and interaction-use shapes.
- `crates/waml/src/uml/analysis.rs` — collect the new typed nodes, construct declared values, and delegate sequence projection to `uml::sequence`.
- `crates/waml/src/uml.rs` — register `mod sequence` and export the declared and typed syntax interfaces.
- `crates/waml/src/model.rs` — replace `MessageVerb` and string endpoints with the runtime types in this plan; add runtime gate and interaction-use pools.
- `crates/waml/src/diagnostic.rs` — add stable sequence validation codes and remove `UnmatchedReply`.
- `crates/waml/src/uml/format.rs` — order and format `## Gates`; keep canonical grammar unchanged.
- `crates/waml/src/solve/interaction.rs` — solve typed endpoints, correlated activations, `par`, boundary endpoints, and interaction-use frames.
- `crates/waml/tests/uml_behavior_syntax.rs` — convert old baseline examples and remove assertions that accepted constructs are deferred.
- `crates/waml/tests/golden.rs` — update declared fingerprints and runtime projection assertions.
- `crates/waml/tests/serde_shape.rs` — pin the replacement JSON shape with no compatibility aliases.
- `crates/waml/tests/interaction_solver_golden.rs` — add the new solver contracts and update constructors.
- `crates/waml/tests/fixtures/behavior/sequence-nested/sequence.md` — use canonical messages.
- `crates/waml/tests/fixtures/behavior/sequence-nested/sequence.golden.txt` — use canonical runtime names and new solver output.
- `crates/waml/tests/fixtures/parser-platform/sequence.md` — use the canonical call form.
- `crates/waml/tests/fixtures/parser-platform/recovery/sequence.md` — replace formerly unsupported valid forms with malformed canonical recovery cases.
- `crates/waml/tests/formatter_actions.rs` — update embedded canonical sequence documents.
- `fuzz/seeds/uml_islands/sequence.md` — use the canonical call form.
- `docs/waml/architecture/views/authoring-and-validation.md` — convert call and return examples.
- `docs/waml/architecture/views/editing-round-trip.md` — convert call and return examples.
- `docs/waml/architecture/views/share-round-trip.md` — convert calls, returns, and signals.
- `docs/superpowers/specs/2026-07-11-uaml-behavioral-substrates-design.md` — convert its live sequence example and terminology.
- `docs/superpowers/specs/2026-07-16-orders-uml-template-split-design.md` — convert its live sequence example.
- `crates/waml-editor/src/behavior_doc_view.rs` — adapt ID and endpoint lookups without adding visual behavior.
- `crates/waml-editor/src/inspector.rs` — render readable labels from canonical message fields.
- `crates/waml-editor/src/canvas/behavior/scene.rs` — rename the carried message enum type.
- `crates/waml-editor/src/canvas/behavior/hit.rs` — update constructors in tests.
- `crates/waml-editor/src/canvas/behavior/render/interaction.rs` — map the replacement message kinds to the current line and arrow styles.

### Locked runtime interfaces

Use these names in every task. Do not introduce a second representation.

```rust
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct MessageId(pub String);

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct InteractionUseId(pub String);

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EndpointRef {
    Lifeline { id: String },
    Outside,
    LocalGate { gate: String },
    UseGate {
        interaction_use: InteractionUseId,
        gate: String,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MessageKind {
    SyncCall,
    AsyncCall,
    AsyncSignal,
    Reply,
    Create,
    Delete,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SeqEdge {
    pub id: MessageId,
    pub from: EndpointRef,
    pub kind: MessageKind,
    pub to: Option<EndpointRef>,
    pub value: Option<String>,
    pub call_id: Option<String>,
    pub returns_call: Option<MessageId>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum OperandSpec {
    Guard(String),
    Else,
    Branch { label: Option<String> },
}

#[derive(Clone, Debug, PartialEq)]
pub struct SeqBinding {
    pub local: String,
    pub target: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SeqInteractionUse {
    pub id: InteractionUseId,
    pub target: String,
    pub alias: String,
    pub bindings: Vec<SeqBinding>,
    pub gates: Vec<String>,
}
```

`SeqChild` gains `InteractionUse { interaction_use: InteractionUseId }`. `SeqNode::Operand` replaces `guard: Option<String>` with `spec: OperandSpec`. `SequenceDoc` gains `gates: Vec<String>` and `interaction_uses: Vec<SeqInteractionUse>`. Serde uses the derived canonical variant names already used by this crate. It has no aliases for the deleted shape.

---

### Task 1: Make `## Gates` a first-class Markdown island

**Files:**
- Modify: `crates/waml-syntax/src/markdown/projection.rs:16-26,260-276`
- Test: `crates/waml-syntax/tests/markdown_extensions.rs:42-123`

**Interfaces:**
- Consumes: `WamlSectionKind` and `waml_kind(source, text_range)`.
- Produces: `WamlSectionKind::Gates` for a top-level `## Gates` heading. The island ends at the next level-1 or level-2 heading.

- [ ] **Step 1: Write the failing Markdown ownership test**

Add `gates_is_a_waml_section_between_lifelines_and_messages` with this source and exact kinds:

```rust
let source = "# S\n## Lifelines\n- [A](./a.md)\n## Gates\n- request\n## Messages\n- @request signals A `start`\n# Next\n";
let parsed = parse(source);
assert_eq!(
    parsed.structure.islands.iter().map(|island| island.kind).collect::<Vec<_>>(),
    [
        WamlSectionKind::Lifelines,
        WamlSectionKind::Gates,
        WamlSectionKind::Messages,
    ]
);
assert_eq!(
    parsed.structure.islands[1].content_range,
    range(
        source.find("## Gates\n").unwrap() + "## Gates\n".len(),
        source.find("## Messages").unwrap(),
    )
);
```

- [ ] **Step 2: Run the focused test and confirm the enum is missing**

Run: `rtk cargo test -p waml-syntax --test markdown_extensions gates_is_a_waml_section_between_lifelines_and_messages -- --exact`

Expected: FAIL because `WamlSectionKind::Gates` does not exist.

- [ ] **Step 3: Add the enum variant and heading match**

Add `Gates` between `Lifelines` and `Messages` in `WamlSectionKind`. Add this arm in `waml_kind`:

```rust
"Gates" => Some(WamlSectionKind::Gates),
```

Update the two existing exhaustive section tests so their source, ranges, island counts, and expected arrays include Gates.

- [ ] **Step 4: Run the Markdown extension suite**

Run: `rtk cargo test -p waml-syntax --test markdown_extensions`

Expected: PASS.

- [ ] **Step 5: Commit the island boundary**

```bash
git add crates/waml-syntax/src/markdown/projection.rs crates/waml-syntax/tests/markdown_extensions.rs
git commit -m "feat: recognize sequence gates section"
```

### Task 2: Parse canonical message forms into fixed slots

**Files:**
- Modify: `crates/waml/src/uml/syntax/kind.rs:2-133`
- Modify: `crates/waml/src/uml/syntax/ast.rs:68-84,225-460`
- Modify: `crates/waml/src/uml/syntax/parser.rs:56-176,431-513,1439-1616`
- Modify: `crates/waml/src/uml/syntax/mod.rs:360-370,542-580`
- Create: `crates/waml/tests/sequence_language_syntax.rs`

**Interfaces:**
- Consumes: `WamlSectionKind::Gates`, `GreenFactory<UmlLanguage>`, fixed-slot missing tokens, and two-space indentation.
- Produces: `MessageSyntax` accessors `source_token`, `verb_token`, `target_token`, `async_token`, `value_token`, `as_token`, `call_id_token`, `to_token`, `return_target_token`, `for_token`, and `return_call_token`.

- [ ] **Step 1: Add one table-driven failing fixed-slot test**

Create `sequence_language_syntax.rs` with the existing `analyze`, `root`, `typed`, and `written` helpers from `uml_behavior_syntax.rs`. Add `canonical_messages_have_fixed_lossless_slots`:

```rust
let authored = "---\ntype: uml.Sequence\n---\n# S\n\n## Messages\n- customer calls order `submit()` as submission\n- order calls worker async `run()` as work\n- order returns `accepted` to customer for submission\n- order signals bus `OrderPlaced`\n- order creates worker: `OrderWorker`\n- order destroys worker\n";
let analysis = analyze([("s.md", authored)]);
let messages = typed::<uml::MessageSyntax>(root(&analysis, "s.md"));
assert_eq!(messages.len(), 6);
assert_eq!(messages[0].target_token().unwrap().text().write_to_string(), "order");
assert_eq!(messages[0].value_token().unwrap().text().write_to_string(), "`submit()`");
assert_eq!(messages[0].call_id_token().unwrap().text().write_to_string(), "submission");
assert_eq!(messages[1].async_token().unwrap().text().write_to_string(), "async");
assert_eq!(messages[2].return_target_token().unwrap().text().write_to_string(), "customer");
assert_eq!(messages[2].return_call_token().unwrap().text().write_to_string(), "submission");
assert_eq!(messages[3].value_token().unwrap().text().write_to_string(), "`OrderPlaced`");
assert_eq!(written(&analysis, "s.md"), authored);
```

- [ ] **Step 2: Run the test and confirm the accessors are missing**

Run: `rtk cargo test -p waml --test sequence_language_syntax canonical_messages_have_fixed_lossless_slots -- --exact`

Expected: FAIL at compile time because the new accessors and syntax kinds do not exist.

- [ ] **Step 3: Replace the legacy eight message slots with canonical slots**

Use this slot order in `MessageSyntax`:

```rust
pub const BULLET_SLOT: usize = 0;
pub const SOURCE_SLOT: usize = 1;
pub const VERB_SLOT: usize = 2;
pub const TARGET_SLOT: usize = 3;
pub const ASYNC_SLOT: usize = 4;
pub const VALUE_SLOT: usize = 5;
pub const AS_SLOT: usize = 6;
pub const CALL_ID_SLOT: usize = 7;
pub const TO_SLOT: usize = 8;
pub const RETURN_TARGET_SLOT: usize = 9;
pub const FOR_SLOT: usize = 10;
pub const RETURN_CALL_SLOT: usize = 11;
pub const COLON_SLOT: usize = 12;
pub const RECOVERY_SLOT: usize = 13;
pub const NEWLINE_SLOT: usize = 14;
```

Add fixed slot nodes and tokens for `MessageAsync`, `MessageValue`, `MessageCallId`, `MessageReturnTarget`, and `MessageReturnCall`. Keep `ColonToken` only for `creates` and `destroys`.

- [ ] **Step 4: Replace `sequence_message` with verb-specific parsing**

Dispatch after the source and verb tokens:

```rust
let tail = match verb {
    "calls" => parse_call_tail(f, text, source, verb_end, content_end, &mut children),
    "returns" => parse_return_tail(f, text, source, verb_end, content_end, &mut children),
    "signals" => parse_signal_tail(f, text, source, verb_end, content_end, &mut children),
    "creates" | "destroys" => {
        parse_other_message_tail(f, text, source, verb_end, content_end, &mut children)
    }
    "replies" | "sends" => return unsupported_message(f, text, start, end, diags),
    _ => return malformed_message(f, text, start, end, diags),
};
```

Each `parse_*_tail` helper has this parameter list and returns `MessageTail { end: usize, valid: bool, recovery: Option<GreenElement<UmlLanguage>> }`. `parse_call_tail` accepts `endpoint`, optional `async`, optional inline code, then optional `as name`. `parse_return_tail` accepts optional inline code, optional `to endpoint`, then optional `for name`. `parse_signal_tail` accepts `endpoint` and optional inline code. Reject `async` after `signals` or `returns`, and reject a colon after `calls`, `returns`, or `signals`. Keep every missing slot as a missing token and put remaining authored bytes in `BehaviorRecovery`.

- [ ] **Step 5: Add recovery cases and removed-spelling cases**

Add `malformed_message_recovers_at_the_next_sibling` with malformed `as`, `to`, `for`, missing endpoints, and a final valid signal. Add `removed_message_spellings_are_not_messages` and assert that `replies`, `sends`, and `- a calls b: `old`` each produce `UnsupportedSequenceForm`, create no `DeclaredMessage`, and do not hide the next canonical message.

- [ ] **Step 6: Run syntax tests**

Run: `rtk cargo test -p waml --test sequence_language_syntax`

Expected: PASS.

- [ ] **Step 7: Commit canonical message syntax**

```bash
git add crates/waml/src/uml/syntax/kind.rs crates/waml/src/uml/syntax/ast.rs crates/waml/src/uml/syntax/parser.rs crates/waml/src/uml/syntax/mod.rs crates/waml/tests/sequence_language_syntax.rs
git commit -m "feat: parse canonical sequence messages"
```

### Task 3: Parse all fragment and operand forms

**Files:**
- Modify: `crates/waml/src/uml/syntax/kind.rs`
- Modify: `crates/waml/src/uml/syntax/ast.rs:432-460`
- Modify: `crates/waml/src/uml/syntax/parser.rs:431-513,1333-1437,1602-1616`
- Test: `crates/waml/tests/sequence_language_syntax.rs`

**Interfaces:**
- Consumes: `SequenceFragmentSyntax`, `SequenceOperandSyntax`, and two-space ownership depth.
- Produces: fragment heads `alt`, `opt`, `loop`, `par`, `break`, `critical`, `assert`, and `neg`; operand keywords `when`, `else`, and `branch`; `branch_label_token() -> Option<SyntaxToken<UmlLanguage>>`.

- [ ] **Step 1: Add a failing nested fragment syntax test**

Use one document that contains all eight heads, a labeled branch, an unlabeled branch, `when`, `else`, and a nested `par`. Assert the exact head sequence and these operand tuples:

```rust
assert_eq!(
    operands.iter().map(|operand| (
        operand.keyword_token().text().write_to_string(),
        operand.guard_token().map(|token| token.text().write_to_string()),
        operand.branch_label_token().map(|token| token.text().write_to_string()),
    )).collect::<Vec<_>>(),
    [
        ("when".into(), Some("`ready`".into()), None),
        ("else".into(), None, None),
        ("branch".into(), None, Some("`payment`".into())),
        ("branch".into(), None, None),
    ]
);
```

- [ ] **Step 2: Run the focused test**

Run: `rtk cargo test -p waml --test sequence_language_syntax all_fragment_heads_and_operand_forms_are_typed -- --exact`

Expected: FAIL because `par`, the four other new heads, and branch labels are not typed.

- [ ] **Step 3: Extend fragment and operand parsing**

Make `FragmentKind::parse` independent of syntax parsing later. At the syntax layer, recognize the exact eight head words. Add `OperandBranchLabel` and `BranchLabelToken`. Use this fixed operand order:

```rust
pub const BULLET_SLOT: usize = 0;
pub const KEYWORD_SLOT: usize = 1;
pub const GUARD_SLOT: usize = 2;
pub const BRANCH_LABEL_SLOT: usize = 3;
pub const RECOVERY_SLOT: usize = 4;
pub const NEWLINE_SLOT: usize = 5;
```

`when` requires inline code. `else` accepts no suffix. `branch` accepts zero or one inline-code label. Unknown operand words stay under the nearest fragment as recovery.

- [ ] **Step 4: Keep deferred fragment words unsupported**

Add a table for `strict`, `seq`, `ignore`, `consider`, and `coregion`. Each line must produce `UnsupportedSequenceForm`, create no fragment node, and leave the next `signals` message typed.

- [ ] **Step 5: Run the syntax suite**

Run: `rtk cargo test -p waml --test sequence_language_syntax`

Expected: PASS.

- [ ] **Step 6: Commit fragment syntax**

```bash
git add crates/waml/src/uml/syntax/kind.rs crates/waml/src/uml/syntax/ast.rs crates/waml/src/uml/syntax/parser.rs crates/waml/tests/sequence_language_syntax.rs
git commit -m "feat: parse sequence fragment operands"
```

### Task 4: Parse gates, interaction uses, and bindings

**Files:**
- Modify: `crates/waml/src/uml/syntax/kind.rs`
- Modify: `crates/waml/src/uml/syntax/ast.rs`
- Modify: `crates/waml/src/uml/syntax/parser.rs:56-176,431-513`
- Modify: `crates/waml/src/uml/syntax/mod.rs:360-370`
- Test: `crates/waml/tests/sequence_language_syntax.rs`

**Interfaces:**
- Consumes: the Gates Markdown island and sequence indentation ownership.
- Produces: `GateSyntax`, `InteractionUseSyntax`, and `BindingSyntax` with fixed tokens for link, alias, local handle, and target handle. Message endpoint tokens preserve `outside`, `@name`, and `use@name` as one authored token.

- [ ] **Step 1: Add a failing lossless reference test**

Use this exact parent and target source:

```markdown
## Gates
- request

## Messages
- ref [Authorize payment](./authorize-payment.md) as auth
  - bind order to caller
  - bind payment to payment
- order calls auth@request `authorize()` as authorization
- auth@request returns `approved` for authorization
```

Assert one gate, one interaction use, two bindings, two messages, exact link/alias/bind accessors, and byte-for-byte `written` output.

- [ ] **Step 2: Run the focused test**

Run: `rtk cargo test -p waml --test sequence_language_syntax gates_refs_and_bindings_have_fixed_lossless_slots -- --exact`

Expected: FAIL because Gates has no UML syntax kind and refs are parsed as malformed messages.

- [ ] **Step 3: Add typed nodes and section parsing**

Map `WamlSectionKind::Gates` to `UmlSyntaxKind::GatesSection`. Parse each non-empty list line as `Gate`. In Messages, dispatch `ref ` before message parsing. An interaction use owns consecutive `bind` lines at exactly one deeper indentation level. Add these accessors:

```rust
impl GateSyntax {
    pub fn name_token(&self) -> SyntaxToken<UmlLanguage>;
}

impl InteractionUseSyntax {
    pub fn link(&self) -> Option<SyntaxNode<UmlLanguage>>;
    pub fn alias_token(&self) -> SyntaxToken<UmlLanguage>;
}

impl BindingSyntax {
    pub fn local_token(&self) -> SyntaxToken<UmlLanguage>;
    pub fn target_token(&self) -> SyntaxToken<UmlLanguage>;
}
```

- [ ] **Step 4: Add local recovery tests**

Test a missing ref link, missing alias, malformed `bind`, malformed `@`, and malformed `use@`. Each bad line must keep its exact bytes. A valid later binding, message, or ref must still get a typed node.

- [ ] **Step 5: Run all syntax and lossless tests**

Run:

```bash
rtk cargo test -p waml --test sequence_language_syntax
rtk cargo test -p waml uml::syntax
```

Expected: PASS.

- [ ] **Step 6: Commit interaction reference syntax**

```bash
git add crates/waml/src/uml/syntax/kind.rs crates/waml/src/uml/syntax/ast.rs crates/waml/src/uml/syntax/parser.rs crates/waml/src/uml/syntax/mod.rs crates/waml/tests/sequence_language_syntax.rs
git commit -m "feat: parse sequence gates and refs"
```

### Task 5: Format only canonical sequence syntax

**Files:**
- Modify: `crates/waml/src/uml/format.rs:329-453`
- Create: `crates/waml/tests/sequence_formatter.rs`
- Modify: `crates/waml/tests/formatter_actions.rs:31-207`

**Interfaces:**
- Consumes: valid canonical syntax and Markdown section ranges.
- Produces: section order `Lifelines`, `Gates`, `Messages`; two-space nested message indentation; unchanged inline code; idempotent canonical output.

- [ ] **Step 1: Add a failing canonical formatter test**

Format a document with Messages before Gates before Lifelines and extra spaces outside inline code. Expect exactly:

```markdown
## Lifelines
- [Order](./order.md) as order
- [Payment](./payment.md) as payment

## Gates
- request

## Messages
- ref [Authorize](./authorize.md) as auth
  - bind order to caller
- order calls payment async `charge( as, to, for )` as charge
- payment returns `approved` to order for charge
```

Reparse it, compare a declared-model fingerprint, format it a second time, and assert zero edits.

- [ ] **Step 2: Run the formatter test**

Run: `rtk cargo test -p waml --test sequence_formatter canonical_sequence_format_is_idempotent_and_semantic -- --exact`

Expected: FAIL because `Gates` is not ordered or canonicalized.

- [ ] **Step 3: Add Gates to section order and canonical section names**

Use `lifelines = 7`, `gates = 8`, `messages = 9`, then shift Members and Layout. Preserve indentation for both Messages and the bind lines that Messages owns. Do not rewrite any verb or move clauses.

- [ ] **Step 4: Test malformed and removed source protection**

Add one test where a malformed canonical line causes zero formatter edits. Add a second test that `sends`, `replies`, and a colon-form call have parser recovery and therefore are not rewritten into canonical syntax.

- [ ] **Step 5: Run formatter tests**

Run:

```bash
rtk cargo test -p waml --test sequence_formatter
rtk cargo test -p waml --test formatter_actions
```

Expected: PASS.

- [ ] **Step 6: Commit formatter support**

```bash
git add crates/waml/src/uml/format.rs crates/waml/tests/sequence_formatter.rs crates/waml/tests/formatter_actions.rs
git commit -m "feat: format canonical sequence syntax"
```

### Task 6: Land the declared and runtime message vertical slice

**Files:**
- Modify: `crates/waml/src/uml/declared.rs:107-147`
- Modify: `crates/waml/src/uml/analysis.rs:377-393,562-587,2917-3015`
- Modify: `crates/waml/src/uml.rs:20-34`
- Create: `crates/waml/src/uml/sequence.rs`
- Modify: `crates/waml/src/model.rs:603-762,1285-1325`
- Modify: `crates/waml/src/diagnostic.rs:16-98`
- Modify: `crates/waml/src/solve/interaction.rs`
- Modify: `crates/waml/tests/serde_shape.rs:307-389`
- Modify: `crates/waml/tests/interaction_solver_golden.rs`
- Modify: `crates/waml-editor/src/behavior_doc_view.rs`
- Modify: `crates/waml-editor/src/inspector.rs`
- Modify: `crates/waml-editor/src/canvas/behavior/scene.rs`
- Modify: `crates/waml-editor/src/canvas/behavior/hit.rs`
- Modify: `crates/waml-editor/src/canvas/behavior/render/interaction.rs`
- Test: `crates/waml/tests/sequence_language_syntax.rs`
- Create: `crates/waml/tests/sequence_semantics.rs`
- Modify: `crates/waml/tests/golden.rs:128-380`

**Interfaces:**
- Consumes: typed syntax nodes from Tasks 2-4 and the locked declared/runtime interfaces below.
- Produces: one compiling vertical slice with authored fields, typed runtime endpoints, canonical runtime kinds, basic endpoint resolution, return correlation, serialized shape, and mechanically updated consumers. The declared, runtime, and lowering shapes change in one commit because no compatible intermediate representation is permitted.

Use these exact declared value types:

```rust
pub enum DeclaredEndpointRef {
    Lifeline(String),
    Outside,
    LocalGate(String),
    UseGate { use_alias: String, gate: String },
}

pub enum DeclaredMessageKind {
    SyncCall,
    AsyncCall,
    AsyncSignal,
    Reply,
    Create,
    Delete,
}

pub enum DeclaredOperandSpec {
    Guard(String),
    Else,
    Branch { label: Option<String> },
}

pub struct DeclaredMessage {
    pub syntax: MessageSyntax,
    pub source: DeclaredField<UmlLanguage, DeclaredEndpointRef>,
    pub kind: DeclaredField<UmlLanguage, DeclaredMessageKind>,
    pub target: DeclaredField<UmlLanguage, DeclaredEndpointRef>,
    pub value: DeclaredField<UmlLanguage, String>,
    pub call_id: DeclaredField<UmlLanguage, String>,
    pub return_to: DeclaredField<UmlLanguage, DeclaredEndpointRef>,
    pub return_for: DeclaredField<UmlLanguage, String>,
    pub depth: usize,
}

pub struct DeclaredGate {
    pub syntax: GateSyntax,
    pub name: DeclaredField<UmlLanguage, String>,
}

pub struct DeclaredFragment {
    pub syntax: SequenceFragmentSyntax,
    pub kind: DeclaredField<UmlLanguage, FragmentKind>,
    pub depth: usize,
}

pub struct DeclaredOperand {
    pub syntax: SequenceOperandSyntax,
    pub spec: DeclaredField<UmlLanguage, DeclaredOperandSpec>,
    pub depth: usize,
}

pub struct DeclaredBinding {
    pub syntax: BindingSyntax,
    pub local: DeclaredField<UmlLanguage, String>,
    pub target: DeclaredField<UmlLanguage, String>,
}

pub struct DeclaredInteractionUse {
    pub syntax: InteractionUseSyntax,
    pub link: DeclaredField<UmlLanguage, String>,
    pub alias: DeclaredField<UmlLanguage, String>,
    pub bindings: Arc<[DeclaredBinding]>,
    pub depth: usize,
}
```

`DeclaredConcept` owns separate ordered arcs named `gates`, `messages`, `fragments`, `operands`, and `interaction_uses` with the exact element types above.

- [ ] **Step 1: Add a failing declared preservation test**

Parse a return with all authored fields and a conflicting `to` value. Assert that declared source, `value`, `return_to`, and `return_for` are all `Valid`. Assert that no return match exists in the declared type.

- [ ] **Step 2: Run the focused declared test**

Run: `rtk cargo test -p waml --test sequence_language_syntax declared_messages_preserve_authored_fields_without_matching -- --exact`

Expected: FAIL because the declared fields do not exist.

- [ ] **Step 3: Replace declared structs and collection**

Add `declared_endpoint(token_text)` that maps exact endpoint spellings. Derive `DeclaredMessageKind` from the verb and the presence of the async token. Keep `DeclaredField::Invalid` for malformed fields. Collect gates, fragments, operands, refs, and bindings by syntax kind. Bind each `BindingSyntax` to its nearest parent `InteractionUseSyntax` by source range and indentation depth.

- [ ] **Step 4: Extend retained-analysis fingerprints**

In `golden.rs`, fingerprint every field of the new declared structs. Include interaction-use binding order. This makes incremental retained analysis compare the full replacement model.

- [ ] **Step 5: Continue the atomic type change**

Do not compile this phase by itself. Continue directly to Phase 6B. The removed declared fields have no permitted adapter to the old runtime enum.

#### Phase 6B: Replace runtime sequence types and serialized shape

**Files:**
- Modify: `crates/waml/src/model.rs:603-762,1285-1325`
- Modify: `crates/waml/src/diagnostic.rs:16-98`
- Modify: `crates/waml/tests/serde_shape.rs:307-389`
- Modify: `crates/waml/tests/golden.rs:672-725`

**Interfaces:**
- Consumes: the locked runtime interfaces at the start of this plan.
- Produces: typed endpoints, stable IDs, six exact message kinds, eight exact fragment kinds, operand specs, gate declarations, and interaction-use pools.

- [ ] **Step 1: Replace the serde test with a failing new-shape assertion**

Use canonical source with a local gate, sync call with `as`, matched return, `par`, and an interaction use. Assert these values:

```rust
assert_eq!(s["edges"][0]["kind"], "syncCall");
assert_eq!(s["edges"][0]["from"], serde_json::json!({"endpoint":"lifeline","id":"buyer"}));
assert_eq!(s["edges"][0]["callId"], "submission");
assert_eq!(s["edges"][1]["kind"], "reply");
assert_eq!(s["edges"][1]["returnsCall"], "m0");
assert_eq!(s["gates"], serde_json::json!(["request"]));
assert_eq!(s["interactionUses"][0]["alias"], "auth");
```

Add a direct serde round-trip value that contains `EndpointRef::Outside`, `EndpointRef::LocalGate`, `EndpointRef::UseGate`, every `MessageKind`, every `FragmentKind`, all three `OperandSpec` variants, and one interaction use. Assert that decode equals the original value.

- [ ] **Step 2: Run the serde test**

Run: `rtk cargo test -p waml --features serde --test serde_shape sequence_doc_json_matches_ts_field_names -- --exact`

Expected: FAIL because the current model serializes `verb`, string endpoints, and the old variants.

- [ ] **Step 3: Implement the locked runtime types**

Rename `MessageVerb` to `MessageKind`. Make `as_str` map `SyncCall` and `AsyncCall` to `calls`, `AsyncSignal` to `signals`, `Reply` to `returns`, `Create` to `creates`, and `Delete` to `destroys`. Use the explicit serde names `syncCall`, `asyncCall`, `asyncSignal`, `reply`, `create`, and `delete`. Extend `FragmentKind` with `Par`, `Break`, `Critical`, `Assert`, and `Neg`. Delete `Sends` and `Replies`; do not add aliases.

Use tagged endpoint JSON with `#[serde(tag = "endpoint", rename_all = "camelCase")]`. Use transparent serde for `MessageId` and `InteractionUseId`. Use `#[serde(rename_all = "camelCase")]` on sequence structs.

- [ ] **Step 4: Add stable diagnostic codes**

Add exact kebab-case strings for `duplicate-sequence-name`, `reserved-sequence-name`, `unknown-sequence-endpoint`, `invalid-sequence-endpoint`, `invalid-lifeline-lifetime`, `duplicate-call-identity`, `unknown-call-identity`, `unmatched-return`, `ambiguous-return`, `completed-return`, `conflicting-return`, `invalid-fragment-operands`, `duplicate-gate`, `invalid-interaction-use`, `interaction-use-cycle`, and `unsupported-sequence-form`. Delete `UnmatchedReply`.

- [ ] **Step 5: Update all compile-time consumers mechanically**

In the current solver, tests, and editor consumers, replace legacy variants with `SyncCall`, `AsyncSignal`, `Reply`, `Create`, and `Delete`. Wrap local endpoint strings as `EndpointRef::Lifeline { id }`. Use `value` instead of `signature`, typed IDs in maps, empty gate/use pools in direct constructors, and `None` for `call_id` and `returns_call` where a test does not exercise correlation. Do not add new visual behavior in this phase.

- [ ] **Step 6: Continue directly to Phase 6C**

Do not commit the runtime type replacement without its lowering implementation. Continue with the same working tree.

#### Phase 6C: Resolve endpoints, calls, returns, and lifetimes

**Files:**
- Create: `crates/waml/src/uml/sequence.rs`
- Modify: `crates/waml/src/uml.rs:3-11`
- Modify: `crates/waml/src/uml/analysis.rs:1433-1543,2141-2322`
- Create: `crates/waml/tests/sequence_semantics.rs`

**Interfaces:**
- Consumes: `DeclaredBundle`, one `DeclaredConcept`, the current document path, and the locked runtime types.
- Produces: `pub(crate) fn lower(context, declared, concept, okf, path, claimed, model, diagnostics)` and a `SequenceDoc` with resolved endpoints and `returns_call` links.

- [ ] **Step 1: Add failing endpoint and message-kind tests**

Build one sequence with lifelines, `outside`, `@request`, and `auth@request`. Assert all four `EndpointRef` variants. Add six messages and assert the exact six `MessageKind` values in source order.

- [ ] **Step 2: Add failing call matching tests**

Test these cases separately: exact `for`; source plus `to`; source-only one-candidate inference; recursion; two overlapping async calls with tags; ambiguous untagged return; unknown tag; reused completed tag; and conflicting source or `to`. For a valid return, assert `returns_call == Some(MessageId("m0".into()))`. For an invalid return, assert that its edge remains and `returns_call == None`.

- [ ] **Step 3: Run the focused semantic tests**

Run:

```bash
rtk cargo test -p waml --test sequence_semantics endpoint_kinds_resolve
rtk cargo test -p waml --test sequence_semantics returns_follow_the_locked_candidate_algorithm
```

Expected: FAIL because `uml::sequence` and runtime lowering do not exist.

- [ ] **Step 4: Move sequence projection into `uml::sequence`**

Remove `lower_sequence_behavior` from `analysis.rs`. Register `mod sequence` in `uml.rs`. Build the ordered tree first, then run endpoint and call resolution. Use this candidate order:

```rust
if let Some(call_id) = authored_for {
    select the unique preceding call with that call_id;
} else {
    select preceding, not-completed calls whose target is the return source;
    if let Some(authored_to) = authored_to {
        retain calls whose source is authored_to;
    }
    require exactly one candidate;
}
```

When both `to` and `for` exist, use `for` to select and use `to` only to validate. Mark a selected call complete after one valid explicit return. Keep source-order edges even when matching fails.

- [ ] **Step 5: Add lifeline name and lifetime validation**

Reject duplicate handles, `outside`, and aliases that contain `@`. Track create and delete rows per local lifeline. Diagnose use before create, use after delete, duplicate create, duplicate delete, and a non-lifeline create/delete target. Keep invalid declared items and keep safe runtime edges.

- [ ] **Step 6: Run the complete vertical-slice tests**

Run:

```bash
rtk cargo test -p waml --test sequence_language_syntax
rtk cargo test -p waml --test sequence_semantics
rtk cargo test -p waml --features serde --test serde_shape
rtk cargo test -p waml --test golden
rtk cargo test -p waml --test interaction_solver_golden
rtk cargo check -p waml-editor
```

Expected: PASS. No intermediate compatibility type or alias exists.

- [ ] **Step 7: Commit message resolution**

```bash
git add crates/waml/src/uml/declared.rs crates/waml/src/uml/analysis.rs crates/waml/src/uml.rs crates/waml/src/uml/sequence.rs crates/waml/src/model.rs crates/waml/src/diagnostic.rs crates/waml/src/solve/interaction.rs crates/waml/tests/sequence_language_syntax.rs crates/waml/tests/sequence_semantics.rs crates/waml/tests/serde_shape.rs crates/waml/tests/golden.rs crates/waml/tests/interaction_solver_golden.rs crates/waml-editor/src/behavior_doc_view.rs crates/waml-editor/src/inspector.rs crates/waml-editor/src/canvas/behavior/scene.rs crates/waml-editor/src/canvas/behavior/hit.rs crates/waml-editor/src/canvas/behavior/render/interaction.rs
git commit -m "feat: resolve canonical sequence messages"
```

### Task 7: Validate fragments and preserve partial order

**Files:**
- Modify: `crates/waml/src/uml/sequence.rs`
- Test: `crates/waml/tests/sequence_semantics.rs`

**Interfaces:**
- Consumes: `DeclaredFragment`, `DeclaredOperand`, source depth, and source ranges.
- Produces: `SeqNode::Fragment`, `SeqNode::Operand { spec, items }`, nested `SeqChild` streams, and validated operand rules.

- [ ] **Step 1: Add failing operand-rule tests**

Create one table with these valid counts: `alt` one or more `when` and optional final `else`; `opt`, `loop`, and `break` exactly one `when`; `par` two or more `branch`; `critical`, `assert`, and `neg` exactly one `branch`. Create invalid controls for wrong words, wrong counts, repeated `else`, non-final `else`, and one-branch `par`.

- [ ] **Step 2: Add a failing ordered-tree test**

Use a nested `par` inside an `alt`. Assert each branch has its own ordered children, the nested fragment stays in its parent operand, and the root item after `par` follows the fragment. Assert no runtime variant exists for deferred fragment words.

- [ ] **Step 3: Run the fragment tests**

Run:

```bash
rtk cargo test -p waml --test sequence_semantics fragment_operand_rules_are_exact
rtk cargo test -p waml --test sequence_semantics nested_fragments_keep_order_and_branch_boundaries
```

Expected: FAIL until the resolver distinguishes guard, else, and branch operands.

- [ ] **Step 4: Implement fragment validation**

Build the nested child streams from source depth before message matching. Preserve invalid fragments in the declared layer. Add runtime fragments only when the kind is typed; keep their valid child items even when an operand-count diagnostic exists. Use `InvalidFragmentOperands` with the authored fragment or operand range.

- [ ] **Step 5: Make return matching branch-aware**

For `par`, give each branch the calls that are open before the fragment. A branch cannot infer a return from a call in a sibling branch. At the join, union calls that can remain open in any branch. A later untagged return is ambiguous when more than one joined call is eligible. Keep stable `for` matching across the join.

- [ ] **Step 6: Run semantic tests and commit**

Run: `rtk cargo test -p waml --test sequence_semantics`

Expected: PASS.

```bash
git add crates/waml/src/uml/sequence.rs crates/waml/tests/sequence_semantics.rs
git commit -m "feat: resolve sequence fragments"
```

### Task 8: Resolve interaction uses, bindings, gates, and cycles

**Files:**
- Modify: `crates/waml/src/uml/sequence.rs`
- Test: `crates/waml/tests/sequence_semantics.rs`

**Interfaces:**
- Consumes: the complete `DeclaredBundle`, resolved href rules, declared lifelines and gates in target sequences.
- Produces: `SeqInteractionUse` pool entries and `SeqChild::InteractionUse` references without flattening target documents.

- [ ] **Step 1: Add failing happy-path reference tests**

Use a parent sequence and an `authorize-payment.md` target. Assert the target key, alias, ordered bindings, referenced gate list, and root `SeqChild::InteractionUse`. Assert that target messages are absent from the parent edge pool.

Add a third sequence that the target references. Assert that the parent use points only to the target key, the target use points only to the third key, and neither referenced edge pool is flattened into its parent.

- [ ] **Step 2: Add failing validation tests**

Add focused cases for a missing document, missing gate, missing participating lifeline binding, duplicate local binding, duplicate target binding, classifier mismatch between bound lifelines, duplicate gate, duplicate use alias, alias collision with a lifeline, unmatched outer gate connection, a direct cycle, and an indirect three-document cycle.

- [ ] **Step 3: Run interaction-use tests**

Run:

```bash
rtk cargo test -p waml --test sequence_semantics interaction_use_resolves_without_flattening
rtk cargo test -p waml --test sequence_semantics invalid_interaction_uses_keep_declared_siblings
```

Expected: FAIL because the runtime use pool is empty.

- [ ] **Step 4: Implement reference resolution**

Resolve the ref link with the current document path. Require a target `uml.Sequence`. Resolve each binding from a parent local lifeline to a target lifeline. Compare the resolved classifier keys when both sides have a classifier. A target lifeline participates when a target edge uses it and it is not first introduced by a target create edge.

- [ ] **Step 5: Implement cycle and gate validation**

Build `BTreeMap<String, Vec<String>>` from resolved interaction-use links. Use a three-state depth-first search and emit `InteractionUseCycle` on each authored ref that enters a cycle. Resolve `alias@gate` only after the alias and target gate exist. Keep invalid refs in the declared bundle and keep later valid root children.

- [ ] **Step 6: Run semantic tests and commit**

Run: `rtk cargo test -p waml --test sequence_semantics`

Expected: PASS.

```bash
git add crates/waml/src/uml/sequence.rs crates/waml/tests/sequence_semantics.rs
git commit -m "feat: resolve sequence interaction uses"
```

### Task 9: Solve typed messages and correlated activations

**Files:**
- Modify: `crates/waml/src/solve/interaction.rs:1-723`
- Modify: `crates/waml/tests/interaction_solver_golden.rs`
- Modify: `crates/waml/tests/fixtures/behavior/sequence-nested/sequence.golden.txt`

**Interfaces:**
- Consumes: typed endpoints, `MessageKind`, stable `MessageId`, and `returns_call`.
- Produces: message rows, self loopbacks, boundary endpoints, create/delete stem bounds, and activation bars paired by message identity.

- [ ] **Step 1: Update constructors, then add failing focused solver tests**

Replace direct string IDs and endpoints in existing constructors. Add tests for a recursive self call and matched return, two overlapping async calls returned in reverse order, one found signal, and one lost signal. Assert exact activation start/end rows and loopback presence.

- [ ] **Step 2: Run the focused solver tests**

Run:

```bash
rtk cargo test -p waml --test interaction_solver_golden correlated_returns_close_the_selected_activation
rtk cargo test -p waml --test interaction_solver_golden found_and_lost_messages_use_frame_edges
```

Expected: FAIL because the solver still uses endpoint strings and a LIFO reply stack.

- [ ] **Step 3: Split row placement from activation derivation**

In pass one, store solved coordinates by `MessageId`. Resolve lifeline endpoint x values from `EndpointRef::Lifeline`. Put `Outside` and `LocalGate` on the nearest outer frame edge based on the local peer x value. Reject `outside` to `outside` before geometry. Keep self messages as two-row loopbacks.

In pass two, for every call edge, find the reply whose `returns_call` equals the call ID. Create the activation on the call target from call y to reply y. If no return exists, extend an unclosed activation to the interaction bottom. `AsyncSignal` creates no activation.

- [ ] **Step 4: Update pretty output**

Print canonical kind names, typed endpoint labels, `returns=<message-id>` for matched replies, and `outside-left` or `outside-right` for boundary endpoints. Keep output deterministic.

- [ ] **Step 5: Run solver tests**

Run: `rtk cargo test -p waml --test interaction_solver_golden`

Expected: PASS for message, activation, self, found, lost, create, and delete contracts.

- [ ] **Step 6: Commit typed message solving**

```bash
git add crates/waml/src/solve/interaction.rs crates/waml/tests/interaction_solver_golden.rs crates/waml/tests/fixtures/behavior/sequence-nested/sequence.golden.txt
git commit -m "feat: solve correlated sequence messages"
```

### Task 10: Solve `par`, new frames, and interaction-use frames

**Files:**
- Modify: `crates/waml/src/solve/interaction.rs`
- Modify: `crates/waml/tests/interaction_solver_golden.rs`
- Modify: `crates/waml/tests/fixtures/behavior/sequence-nested/sequence.golden.txt`

**Interfaces:**
- Consumes: `OperandSpec`, all `FragmentKind` variants, use-gate endpoints, `SeqInteractionUse`, and use children.
- Produces: deterministic branch-local rows, a `par` join, all frame kinds, and `SolvedInteractionUse`/`SolvedGate` geometry in `SolvedInteraction`.

- [ ] **Step 1: Add a failing `par` geometry test**

Build two branches with two messages each and a root message after the fragment. Assert both branches start at the same y, each branch keeps increasing y, and the root message y is greater than both branch end rows.

- [ ] **Step 2: Add failing frame and reference golden tests**

Assert `break`, `critical`, `assert`, and `neg` appear in pretty output. Add one interaction use with two gates and two bindings. Assert one use frame, gate coordinates on its frame boundary, outer messages connected to those coordinates, and no copied target messages.

- [ ] **Step 3: Run the focused tests**

Run:

```bash
rtk cargo test -p waml --test interaction_solver_golden par_branches_share_a_start_and_join_after_all_branches
rtk cargo test -p waml --test interaction_solver_golden interaction_use_frames_keep_bindings_and_gates
```

Expected: FAIL because `walk_items` has no interaction-use arm and all operands advance one shared y cursor.

- [ ] **Step 4: Implement branch-local solving**

For `FragmentKind::Par`, save the branch start y. Reset y to that value for each operand. Record each branch end and set the post-fragment y to the maximum end plus closing padding. For all other fragment kinds, keep sequential operand layout. Render `Branch` labels without brackets and guards with brackets.

- [ ] **Step 5: Implement use-frame solving**

Add these exact solver output types:

```rust
pub struct SolvedGate {
    pub name: String,
    pub x: f64,
    pub y: f64,
}

pub struct SolvedInteractionUse {
    pub id: InteractionUseId,
    pub target: String,
    pub rect: Rect,
    pub bindings: Vec<SeqBinding>,
    pub gates: Vec<SolvedGate>,
}
```

Add `interaction_uses: Vec<SolvedInteractionUse>` to `SolvedInteraction`. Reserve one ordered frame row for the ref item. Precompute use frames before endpoint resolution so messages before or after the ref can resolve `UseGate` coordinates. Add the new pool to `pretty_interaction`.

- [ ] **Step 6: Run solver tests and commit**

Run: `rtk cargo test -p waml --test interaction_solver_golden`

Expected: PASS.

```bash
git add crates/waml/src/solve/interaction.rs crates/waml/tests/interaction_solver_golden.rs crates/waml/tests/fixtures/behavior/sequence-nested/sequence.golden.txt
git commit -m "feat: solve sequence frames and refs"
```

### Task 11: Pin non-visual runtime consumers

**Files:**
- Modify: `crates/waml-editor/src/behavior_doc_view.rs:287-421,501-651`
- Modify: `crates/waml-editor/src/inspector.rs:333-380`
- Modify: `crates/waml-editor/src/canvas/behavior/scene.rs:5-100,285-305`
- Modify: `crates/waml-editor/src/canvas/behavior/hit.rs:259-305`
- Modify: `crates/waml-editor/src/canvas/behavior/render/interaction.rs:18,394-426`

**Interfaces:**
- Consumes: `MessageKind`, `MessageId`, `EndpointRef`, and current solved message geometry from Task 6.
- Produces: exact readable labels and the same current editor scene. This task adds no controls, ref-frame drawing, gate drawing, or new editing actions.

- [ ] **Step 1: Add failing label and style tests**

In `inspector.rs`, add a table that expects labels for a lifeline call, an outside signal, a local-gate message, a use-gate message, and a return with a value. In `render/interaction.rs`, extract `message_style(kind) -> (bool, bool)` and test dashed and filled-head values for all six `MessageKind` variants.

Run:

```bash
rtk cargo test -p waml-editor inspector::tests::canonical_sequence_endpoint_labels
rtk cargo test -p waml-editor canvas::behavior::render::interaction::tests::canonical_message_styles
```

Expected: FAIL because endpoint label formatting and the six-kind style table are not yet pinned.

- [ ] **Step 2: Implement the complete style mapping**

Keep `MessageKind` in scene and rendering. Make `message_style` return dashed lines for `Reply` and `Create`. Return filled heads for `SyncCall`, `AsyncCall`, and `Delete`. Return open heads for `AsyncSignal`, `Reply`, and `Create`.

- [ ] **Step 3: Add one endpoint display helper**

Use this exact mapping in inspector labels:

```rust
fn endpoint_label(endpoint: &EndpointRef, lifeline_title: &dyn Fn(&str) -> String) -> String {
    match endpoint {
        EndpointRef::Lifeline { id } => lifeline_title(id),
        EndpointRef::Outside => "outside".into(),
        EndpointRef::LocalGate { gate } => format!("@{gate}"),
        EndpointRef::UseGate { interaction_use, gate } => {
            format!("{}@{gate}", interaction_use.0)
        }
    }
}
```

Render returns as `source returns [value] [to target]` and other messages as `source kind target [value]`. Use `.0.as_str()` or an `as_str()` method for typed IDs in maps.

- [ ] **Step 4: Run editor unit tests and check**

Run:

```bash
rtk cargo test -p waml-editor --lib
rtk cargo check -p waml-editor
```

Expected: PASS. There are no new visual behavior assertions.

- [ ] **Step 5: Commit consumer updates**

```bash
git add crates/waml-editor/src/behavior_doc_view.rs crates/waml-editor/src/inspector.rs crates/waml-editor/src/canvas/behavior/scene.rs crates/waml-editor/src/canvas/behavior/hit.rs crates/waml-editor/src/canvas/behavior/render/interaction.rs
git commit -m "refactor: adapt sequence model consumers"
```

### Task 12: Convert the repository corpus and run the done bar

**Files:**
- Modify: `crates/waml/tests/fixtures/behavior/sequence-nested/sequence.md`
- Modify: `crates/waml/tests/fixtures/behavior/sequence-nested/sequence.golden.txt`
- Modify: `crates/waml/tests/fixtures/parser-platform/sequence.md`
- Modify: `crates/waml/tests/fixtures/parser-platform/recovery/sequence.md`
- Modify: `fuzz/seeds/uml_islands/sequence.md`
- Modify: `docs/waml/architecture/views/authoring-and-validation.md`
- Modify: `docs/waml/architecture/views/editing-round-trip.md`
- Modify: `docs/waml/architecture/views/share-round-trip.md`
- Modify: `docs/superpowers/specs/2026-07-11-uaml-behavioral-substrates-design.md`
- Modify: `docs/superpowers/specs/2026-07-16-orders-uml-template-split-design.md`
- Modify: `crates/waml/tests/uml_behavior_syntax.rs`
- Modify: `crates/waml/tests/golden.rs`
- Modify: `crates/waml/tests/formatter_actions.rs`
- Modify: `crates/waml/tests/serde_shape.rs`

**Interfaces:**
- Consumes: the canonical grammar and new serialized/runtime names.
- Produces: no legacy authored sequence message in active repository documents or expectations.

- [ ] **Step 1: Convert every known authored sequence line**

Apply these exact transformations:

```text
- a calls b: `op`       -> - a calls b `op`
- b replies a: `value` -> - b returns `value` to a
- a sends b: `signal`  -> - a signals b `signal`
```

For the nested fixture, use these exact lines so activation nesting stays deterministic:

```markdown
- a calls b `start()`
- b calls c `work()`
- c returns `done` to b
- b returns `ok` to a
- d returns `ack` to b
```

Do not change `creates` or `destroys` endpoint-first syntax.

- [ ] **Step 2: Replace obsolete recovery controls**

In `parser-platform/recovery/sequence.md`, do not keep valid `par` or self-signal lines as unsupported controls. Use malformed canonical controls such as `- par` with `- when` instead of branches, `- target calls`, `- target returns to`, and a malformed `bind`. Keep a later valid `- target signals target` line to prove recovery.

- [ ] **Step 3: Update baseline assertions and goldens**

In `uml_behavior_syntax.rs`, replace the old deferred test with two tests: accepted self/`par`/outside/gate/ref forms, and rejected deferred or removed spellings. In `golden.rs`, assert `MessageKind::SyncCall` and typed endpoint values. Update formatter and serde embedded strings to canonical syntax.

- [ ] **Step 4: Run the zero-legacy corpus check**

Run:

```bash
rtk rg -n --glob '!target/**' --glob '!.worktrees/**' --glob '!docs/superpowers/plans/**' '^\s*-\s+\S+\s+(replies|sends)\s+|^\s*-\s+\S+\s+calls\s+\S+\s*:' .
```

Expected: exit 1 with no matches. Removed spellings remain only in explicit negative-test string tables and in the approved specification text that describes the replacement policy.

- [ ] **Step 5: Run focused suites**

Run:

```bash
rtk cargo test -p waml-syntax --test markdown_extensions
rtk cargo test -p waml --test sequence_language_syntax
rtk cargo test -p waml --test sequence_formatter
rtk cargo test -p waml --test sequence_semantics
rtk cargo test -p waml --test interaction_solver_golden
rtk cargo test -p waml --features serde --test serde_shape
rtk cargo test -p waml --test formatter_actions
rtk cargo test -p waml --test golden
```

Expected: PASS.

- [ ] **Step 6: Run full relevant verification**

Run:

```bash
rtk cargo fmt --all -- --check
rtk cargo test -p waml-syntax
rtk cargo test -p waml --all-features
rtk cargo test -p waml-editor --lib
rtk cargo check --workspace --all-targets --all-features
rtk cargo clippy --workspace --all-targets --all-features -- -D warnings
```

Expected: every command PASS with no warning promoted by Clippy.

- [ ] **Step 7: Confirm the worktree contains no unrelated edit**

Run: `rtk git status --short` and `rtk git diff --stat`

Expected: the two pre-existing modified files remain untouched. All other changed files belong to this plan.

- [ ] **Step 8: Commit the atomic corpus conversion**

```bash
git add crates/waml/tests/fixtures crates/waml/tests/uml_behavior_syntax.rs crates/waml/tests/golden.rs crates/waml/tests/formatter_actions.rs crates/waml/tests/serde_shape.rs fuzz/seeds/uml_islands/sequence.md docs/waml/architecture/views docs/superpowers/specs/2026-07-11-uaml-behavioral-substrates-design.md docs/superpowers/specs/2026-07-16-orders-uml-template-split-design.md
git commit -m "docs: convert sequence corpus to canonical syntax"
```

## Final acceptance map

- Message grammar and fixed slots: Tasks 2 and 6.
- Self, found, lost, local gates, and use gates: Tasks 2, 4, 6, and 9.
- All eight fragment kinds and operand rules: Tasks 3, 7, and 10.
- Interaction uses, bindings, gates, missing targets, and cycles: Tasks 4, 6, 8, and 10.
- Lossless recovery after every listed malformed form: Tasks 2-4.
- Formatter idempotence and declared parse-format-parse equality: Task 5.
- Canonical runtime kinds, typed endpoints, stable call IDs, and serialization: Task 6.
- Exact call-return candidate algorithm, recursion, overlapping async calls, and ambiguity: Task 6.
- Create/delete lifetime validation: Task 6.
- Solver loopbacks, activations, boundary endpoints, branch joins, frames, refs, and gates: Tasks 9 and 10.
- No compatibility path and complete repository conversion: Task 12.
- No visual editor project: Task 11 pins existing display behavior only.
