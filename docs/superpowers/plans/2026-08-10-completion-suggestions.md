# Completion Suggestions Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Offer a completion list at every position where the WAML grammar expects an operand, derived from the syntax tree, and serve it over the LSP so VS Code completes incomplete constructs.

**Architecture:** A new `crates/waml/src/uml/complete.rs` sibling to `repair.rs`, split into two halves that do not know about each other. A *locator* answers "where am I": it finds the token to the left of the cursor, and produces an `Expectation { slot, token, node, prefix }` from the parser's fixed slots — a zero-width missing token at a real position *is* the statement "an operand belongs here". *Candidate providers* answer "what goes here": one small function per candidate family, selected on the slot/token kind pair. No second description of the grammar is introduced; the keyword tables move to one owner that the parser, the formatter and the providers all read.

**Tech Stack:** Rust (workspace crates `waml`, `waml-cli`), `waml_syntax` Roslyn-style green/red trees, `tower-lsp-server` for the LSP, a TypeScript VS Code extension that needs no change (it is a thin `LanguageClient` shell).

**Spec:** `docs/superpowers/specs/2026-08-10-completion-suggestions-design.md`. Read it before starting. Its decisions were agreed with the user — do not re-litigate them.

## Global Constraints

- **The gate is all of:** `cargo test --workspace`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo fmt --all -- --check`, and — from `editors/vscode` — `pnpm build`, `pnpm test`, `pnpm lint`. Every task must leave all six green on its own.
- **`-D warnings` promotes `dead_code` to a hard error.** In `waml` (a library crate) a `pub` item re-exported from a `pub mod` is publicly reachable and never dead. In `waml-cli` (a **bin-only** crate — there is no `src/lib.rs`) `pub` buys nothing: every new function there needs a real non-test caller in the same commit. Same for `waml-editor`, whose `mod documents;` is crate-private by deliberate policy (see the dead-code note at the top of `crates/waml-editor/src/lib.rs`). Private helpers in `waml` also need callers. Draw every task boundary so the code and its consumer land together.
- **Never commit `crates/waml-syntax/tests/properties.proptest-regressions`.** The randomized proptest in `waml-syntax` appends to that file and can go red on a fresh seed. That failure is **pre-existing and unrelated to this work** — do not chase it, do not fix it here, and do not commit the file if it appears. `git status` it away before every commit.
- **Commit messages: subject + body only. No `Co-Authored-By` trailer**, no `Generated with` footer.
- **Keyword tables are MOVED, never copied.** Task 1 creates the single owner. If a later task needs a keyword list, it reads `uml::vocabulary` — adding a third copy is the exact failure this work exists to stop.
- **Completion never reads diagnostics, and diagnostics never read completion.** Both read the same slot-kind tables. Parser diagnostic wording is final; do not touch it.
- Offsets everywhere in `complete.rs` are **document-absolute**. The UML syntax tree for a document spans the whole file (prose and frontmatter land in `MarkdownRegion`), exactly as `repair.rs` already assumes.
- An unfinished document is the **expected input**, not a failure. Only an unknown document, a catalog mismatch or a revision mismatch is an `Err`; everything else returns an empty `Vec`.

## File Structure

| File | Responsibility |
| --- | --- |
| `crates/waml/src/uml/vocabulary.rs` (new) | The single owner of WAML's closed keyword vocabularies. Read by the parser, the formatter and the completion providers. |
| `crates/waml/src/uml/complete.rs` (new) | `Completion`, `CompletionKind`, `Expectation`, the locator `expectation_at`, the `completions` entry point, and one private provider function per candidate family. |
| `crates/waml/src/uml.rs` | Module wiring and public re-exports. |
| `crates/waml/src/uml/declared.rs` | `ExpectedSyntax` gains five variants plus `expected_for_slot`, the slot-kind table both halves read. |
| `crates/waml/src/uml/analysis.rs` | Call sites switch to `expected_for_slot`; fixes the `declared_lifeline` mislabel. |
| `crates/waml/src/uml/format.rs` | `canonical_layout_keywords` stops owning a keyword list and reads `vocabulary`. |
| `crates/waml/src/uml/syntax/parser.rs` | Layout and message-verb literal match arms read `vocabulary`. |
| `crates/waml/src/analysis.rs` | Re-exports `waml_syntax::{TextRange, TextSize}` so consumers can name the types the public API already uses. |
| `crates/waml-cli/src/lsp/query.rs` | `LspAnalysisState::completion(physical, position)`, beside `definition`. |
| `crates/waml-cli/src/lsp/server.rs` | Advertises `completion_provider`; the `completion` handler. |
| `crates/waml/tests/uml_vocabulary.rs` (new) | The vocabulary tables agree with the grammar and with the model enums. |
| `crates/waml/tests/uml_completion_locator.rs` (new) | The cursor-marker locator table. |
| `crates/waml/tests/uml_completions.rs` (new) | Provider tests, silence tests, error tests. |
| `crates/waml/tests/uml_completion_accept_set.rs` (new) | The accept-set invariant property test. |
| `crates/waml-cli/tests/lsp_e2e.rs` | `textDocument/completion` round trip. |

---

## Slice map

| Slice | Tasks | Leaves working |
| --- | --- | --- |
| 1. Locator, `Completion`, `fixed_vocabulary`, one keyword owner | 1–5 | A library API with keyword completions |
| 2. LSP wiring | 6–7 | **Working completions in VS Code** |
| 3. `in_document_refs` + accept-set invariant | 8–10 | In-document handle/id/target completions |
| 4. `link_targets`, `type_driven`, `derived_names` | 11–13 | The full candidate set |
| 5. Native editor popup | **deferred — see "Deferred work" at the end** | — |

Stopping after Task 7 leaves a coherent, useful feature. Slices are ordered by the spec; keep that order.

---

### Task 1: One owner for the keyword vocabularies

**Files:**
- Create: `crates/waml/src/uml/vocabulary.rs`
- Modify: `crates/waml/src/uml.rs` (add `pub mod vocabulary;`)
- Modify: `crates/waml/src/uml/format.rs:445-516` (delete the local `KEYWORDS` const, read `vocabulary`)
- Modify: `crates/waml/src/uml/lower.rs:1080-1094` (delete the local list, delegate to `vocabulary`)
- Test: `crates/waml/tests/uml_vocabulary.rs` (new)

**Interfaces:**
- Produces: `waml::uml::vocabulary` with `LAYOUT_KEYWORDS`, `LAYOUT_SHAPE_HINTS`, `LAYOUT_MARGIN_SIZES`, `LAYOUT_HINT_PHRASES`, `LAYOUT_EDGE_WORDS`, `LAYOUT_AXIS_WORDS`, `LAYOUT_DIRECTION_HEADS`, `LAYOUT_DIRECTION_PHRASES`, `MESSAGE_VERBS`, `RELATIONSHIP_KINDS`, `FLOW_NODE_KINDS`, `FRAGMENT_KINDS`, `canonical_layout_keyword`, `relationship_keywords`, `flow_node_keywords`, `fragment_keywords`.
- Consumes: `crate::model::{FlowNodeKind, FragmentKind, RelationshipKind}`.

- [ ] **Step 1: Write the failing test**

Create `crates/waml/tests/uml_vocabulary.rs`:

```rust
use waml::uml::vocabulary;

#[test]
fn layout_keywords_are_sorted_unique_and_cover_every_phrase_word() {
    let mut sorted = vocabulary::LAYOUT_KEYWORDS.to_vec();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(sorted.as_slice(), vocabulary::LAYOUT_KEYWORDS);

    // Every word of every phrase table is itself a keyword, so the formatter's
    // word-by-word canonicalisation can never lower-case a phrase only halfway.
    for phrase in vocabulary::LAYOUT_HINT_PHRASES
        .iter()
        .chain(vocabulary::LAYOUT_DIRECTION_PHRASES)
    {
        for word in phrase.split(' ') {
            assert!(
                vocabulary::LAYOUT_KEYWORDS.contains(&word),
                "{phrase:?} uses non-keyword {word:?}"
            );
        }
    }
    for word in vocabulary::LAYOUT_EDGE_WORDS
        .iter()
        .chain(vocabulary::LAYOUT_AXIS_WORDS)
        .chain(vocabulary::LAYOUT_DIRECTION_HEADS)
        .chain(vocabulary::LAYOUT_SHAPE_HINTS)
        .chain(vocabulary::LAYOUT_MARGIN_SIZES)
    {
        assert!(
            vocabulary::LAYOUT_KEYWORDS.contains(word),
            "{word:?} is used by the grammar but is not a layout keyword"
        );
    }
}

#[test]
fn canonical_layout_keyword_lowercases_and_folds_margins() {
    assert_eq!(vocabulary::canonical_layout_keyword("ABOVE"), Some("above"));
    assert_eq!(vocabulary::canonical_layout_keyword("margins"), Some("margin"));
    assert_eq!(vocabulary::canonical_layout_keyword("Margins"), Some("margin"));
    assert_eq!(vocabulary::canonical_layout_keyword("Order"), None);
}

#[test]
fn model_derived_vocabularies_list_every_variant_exactly_once() {
    let relationships = vocabulary::relationship_keywords().collect::<Vec<_>>();
    assert_eq!(relationships.len(), 11);
    for keyword in &relationships {
        assert!(
            waml::model::RelationshipKind::parse(keyword).is_some(),
            "{keyword:?} does not round-trip through RelationshipKind::parse"
        );
    }
    // `instance of` must be offered -- and matched -- before any single word,
    // so a two-word kind is never truncated to a one-word prefix.
    assert_eq!(relationships[0], "instance of");

    let flow = vocabulary::flow_node_keywords().collect::<Vec<_>>();
    assert_eq!(flow.len(), 7);
    assert!(!flow.contains(&"plain"), "Plain has no keyword");
    for keyword in &flow {
        assert!(waml::model::FlowNodeKind::from_keyword(keyword).is_some(), "{keyword}");
    }

    let fragments = vocabulary::fragment_keywords().collect::<Vec<_>>();
    assert_eq!(fragments.len(), 8);
    for keyword in &fragments {
        assert!(waml::model::FragmentKind::parse(keyword).is_some(), "{keyword}");
    }
}

#[test]
fn message_verbs_are_the_five_the_grammar_accepts() {
    assert_eq!(
        vocabulary::MESSAGE_VERBS,
        ["calls", "returns", "signals", "creates", "destroys"]
    );
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p waml --test uml_vocabulary`
Expected: FAIL — `could not find vocabulary in uml`.

- [ ] **Step 3: Create the vocabulary module**

Create `crates/waml/src/uml/vocabulary.rs`:

```rust
//! The single owner of WAML's closed keyword vocabularies.
//!
//! Before this module the layout keyword list existed twice -- as literal match
//! arms in `uml::syntax::parser` and as a private `KEYWORDS` const in
//! `uml::format` -- and the two drifted. The parser, the formatter and
//! `uml::complete` all read the tables here now; a third copy anywhere is a
//! defect. Vocabularies a model enum already owns (`RelationshipKind`,
//! `FlowNodeKind`, `FragmentKind`) are *derived* from that enum rather than
//! retyped, so the enum stays the authority and this module stays the single
//! lookup surface.

use crate::model::{FlowNodeKind, FragmentKind, RelationshipKind};

/// Every word the `## Layout` grammar treats as a keyword rather than a name.
/// Sorted, so the table is greppable and the sortedness test is meaningful.
pub const LAYOUT_KEYWORDS: &[&str] = &[
    "above",
    "aligned",
    "and",
    "as",
    "below",
    "bottom",
    "box",
    "center",
    "collapsed",
    "column",
    "emphasized",
    "frame",
    "large",
    "left",
    "margin",
    "margins",
    "medium",
    "no",
    "of",
    "right",
    "row",
    "shrink",
    "small",
    "top",
    "with",
];

/// Hint words that stand alone after `with`.
pub const LAYOUT_SHAPE_HINTS: &[&str] = &["frame", "box", "shrink", "emphasized", "collapsed"];

/// Hint words that must be followed by `margin` or `margins`.
pub const LAYOUT_MARGIN_SIZES: &[&str] = &["no", "small", "medium", "large"];

/// Complete hint phrases, exactly as a completion inserts them.
pub const LAYOUT_HINT_PHRASES: &[&str] = &[
    "frame",
    "box",
    "shrink",
    "emphasized",
    "collapsed",
    "no margin",
    "small margin",
    "medium margin",
    "large margin",
];

/// Words that open an alignment edge; always followed by `of`.
pub const LAYOUT_EDGE_WORDS: &[&str] = &["top", "bottom", "left", "right", "center"];

/// Words that open an inline group; always followed by `of`.
pub const LAYOUT_AXIS_WORDS: &[&str] = &["row", "column"];

/// Words that open a direction clause.
pub const LAYOUT_DIRECTION_HEADS: &[&str] = &["above", "below", "left", "right"];

/// Complete direction phrases, exactly as a completion inserts them.
pub const LAYOUT_DIRECTION_PHRASES: &[&str] = &[
    "above",
    "below",
    "left of",
    "right of",
    "above left of",
    "above right of",
    "below left of",
    "below right of",
];

/// The five message verbs the `## Messages` grammar accepts.
pub const MESSAGE_VERBS: &[&str] = &["calls", "returns", "signals", "creates", "destroys"];

/// Every relationship kind, longest keyword first. `InstanceOf` leads because
/// `instance of` is two words: matched after the one-word kinds it would be
/// truncated to `instance`. `uml::lower` depends on this order.
pub const RELATIONSHIP_KINDS: &[RelationshipKind] = &[
    RelationshipKind::InstanceOf,
    RelationshipKind::Associates,
    RelationshipKind::Aggregates,
    RelationshipKind::Composes,
    RelationshipKind::Specializes,
    RelationshipKind::Implements,
    RelationshipKind::Depends,
    RelationshipKind::Annotates,
    RelationshipKind::Includes,
    RelationshipKind::Extends,
    RelationshipKind::Links,
];

/// Every flow node kind that has a heading keyword. `Plain` is the absence of
/// one and is deliberately not here.
pub const FLOW_NODE_KINDS: &[FlowNodeKind] = &[
    FlowNodeKind::Initial,
    FlowNodeKind::Final,
    FlowNodeKind::Decision,
    FlowNodeKind::Merge,
    FlowNodeKind::Fork,
    FlowNodeKind::Join,
    FlowNodeKind::Object,
];

/// Every combined-fragment kind.
pub const FRAGMENT_KINDS: &[FragmentKind] = &[
    FragmentKind::Alt,
    FragmentKind::Opt,
    FragmentKind::Loop,
    FragmentKind::Par,
    FragmentKind::Break,
    FragmentKind::Critical,
    FragmentKind::Assert,
    FragmentKind::Neg,
];

pub fn relationship_keywords() -> impl Iterator<Item = &'static str> {
    RELATIONSHIP_KINDS.iter().map(|kind| kind.as_str())
}

pub fn flow_node_keywords() -> impl Iterator<Item = &'static str> {
    FLOW_NODE_KINDS.iter().filter_map(|kind| kind.keyword())
}

pub fn fragment_keywords() -> impl Iterator<Item = &'static str> {
    FRAGMENT_KINDS.iter().map(|kind| kind.as_str())
}

/// The canonical spelling of `word` when it is a layout keyword, `None` when it
/// is a name. `margins` folds to `margin`; everything else lower-cases.
pub fn canonical_layout_keyword(word: &str) -> Option<&'static str> {
    let lower = word.to_ascii_lowercase();
    if lower == "margins" {
        return Some("margin");
    }
    LAYOUT_KEYWORDS
        .iter()
        .copied()
        .find(|keyword| *keyword == lower.as_str())
}
```

- [ ] **Step 4: Register the module**

In `crates/waml/src/uml.rs`, add `pub mod vocabulary;` to the module list (alphabetical, after `pub mod syntax;`).

- [ ] **Step 5: Make the formatter read it**

In `crates/waml/src/uml/format.rs`, delete the `const KEYWORDS: &[&str] = &[...]` block inside `canonical_layout_keywords` and replace the word-matching branch:

```rust
        if quote.is_none() && ch.is_ascii_alphabetic() {
            let end = value[cursor..]
                .find(|next: char| !next.is_ascii_alphabetic())
                .map(|relative| cursor + relative)
                .unwrap_or(value.len());
            let word = &value[cursor..end];
            match crate::uml::vocabulary::canonical_layout_keyword(word) {
                Some(canonical) => out.push_str(canonical),
                None => out.push_str(word),
            }
            cursor = end;
            continue;
        }
```

- [ ] **Step 6: Make the lowerer read it**

In `crates/waml/src/uml/lower.rs`, replace the body of `relationship_kinds`:

```rust
fn relationship_kinds() -> &'static [RelationshipKind] {
    crate::uml::vocabulary::RELATIONSHIP_KINDS
}
```

- [ ] **Step 7: Run the gate**

Run: `cargo test -p waml --test uml_vocabulary`
Expected: PASS

Run: `cargo test --workspace && cargo clippy --workspace --all-targets --all-features -- -D warnings && cargo fmt --all -- --check`
Expected: all green. `crates/waml/tests/formatter_actions.rs` and `sequence_formatter.rs` exercise the layout canonicaliser; if either fails, the vocabulary table lost a word — diff it against the deleted `KEYWORDS` const.

- [ ] **Step 8: Commit**

```bash
git add crates/waml/src/uml/vocabulary.rs crates/waml/src/uml.rs crates/waml/src/uml/format.rs crates/waml/src/uml/lower.rs crates/waml/tests/uml_vocabulary.rs
git commit -m "feat(uml): give the keyword vocabularies one owner

The layout keyword list existed twice: as literal match arms in the
parser and as a private const in the formatter. uml::vocabulary now owns
every closed vocabulary; the formatter and the lowerer read it, and the
model enums stay the authority for the words they already define."
```

---

### Task 2: The parser reads the vocabulary tables

**Files:**
- Modify: `crates/waml/src/uml/syntax/parser.rs` — the layout cursor at `:3376-3529` (`reference`, `hint`, `anchored`, `direction`) and the message verb tests at `:1959-1994` and `:2677`
- Test: `crates/waml/tests/uml_vocabulary.rs` (append)

**Interfaces:**
- Consumes: `crate::uml::vocabulary::{LAYOUT_SHAPE_HINTS, LAYOUT_MARGIN_SIZES, LAYOUT_EDGE_WORDS, LAYOUT_AXIS_WORDS, LAYOUT_DIRECTION_HEADS, MESSAGE_VERBS}` from Task 1.
- Produces: nothing new. This task removes copies.

- [ ] **Step 1: Write the failing test**

Append to `crates/waml/tests/uml_vocabulary.rs`:

```rust
use waml::analysis::prepare_candidate;
use waml::source::SourceBundle;

fn diagnostics_for(text: &str) -> Vec<String> {
    let bundle = SourceBundle::try_from_pairs([("doc.md", text)]).unwrap();
    prepare_candidate(bundle, None, 1)
        .unwrap()
        .uml()
        .diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.clone())
        .collect()
}

#[test]
fn every_layout_hint_phrase_parses_without_a_diagnostic() {
    for phrase in vocabulary::LAYOUT_HINT_PHRASES {
        let text = format!(
            "---\ntype: Diagram\ntitle: D\nprofile: uml-domain\n---\n# D\n\n## Members\n- [A](./a.md)\n\n## Layout\n- A with {phrase}\n"
        );
        let diagnostics = diagnostics_for(&text);
        assert!(
            diagnostics.iter().all(|message| !message.contains("layout")),
            "{phrase:?} produced {diagnostics:?}"
        );
    }
}

#[test]
fn every_message_verb_parses_without_a_malformed_message_diagnostic() {
    for verb in vocabulary::MESSAGE_VERBS {
        let tail = if *verb == "returns" { " to B" } else { "" };
        let text = format!(
            "---\ntype: uml.Sequence\ntitle: S\n---\n# S\n\n## Lifelines\n\n- [A](./a.md) as A\n- [B](./b.md) as B\n\n## Messages\n\n- A {verb} B{tail}\n"
        );
        let diagnostics = diagnostics_for(&text);
        assert!(
            diagnostics
                .iter()
                .all(|message| !message.contains("message")),
            "{verb:?} produced {diagnostics:?}"
        );
    }
}
```

- [ ] **Step 2: Run test to verify it passes today**

Run: `cargo test -p waml --test uml_vocabulary`
Expected: PASS. These two tests pin *current* parser behaviour so Step 3's refactor cannot change it. If a case fails before you touch the parser, the fixture is wrong (usually a missing `to B` tail on `returns`) — fix the fixture, not the parser.

- [ ] **Step 3: Replace the parser's literal keyword arms**

In `crates/waml/src/uml/syntax/parser.rs`, add `use crate::uml::vocabulary;` to the imports at the top, then:

`fn hint(&mut self, recovery_from: usize)` — replace the two literal arms:

```rust
        match word {
            word if vocabulary::LAYOUT_SHAPE_HINTS.contains(&word) => {
                self.pos += 1;
                Ok(())
            }
            word if vocabulary::LAYOUT_MARGIN_SIZES.contains(&word) => {
                let size = word.to_string();
                self.pos += 1;
                if self.eat("margin") || self.eat("margins") {
                    Ok(())
                } else {
                    Err(self.error(
                        recovery_from,
                        self.pos,
                        UmlSyntaxKind::LayoutKeywordToken,
                        LayoutExpectation::Keyword {
                            after: size,
                            keyword: "margin",
                        },
                    ))
                }
            }
            _ => Err(self.error(
                recovery_from,
                self.pos,
                UmlSyntaxKind::LayoutWordToken,
                LayoutExpectation::UnknownHint(word.to_string()),
            )),
        }
```

`fn anchored(&mut self)` — replace the `matches!` edge test:

```rust
        let has_edge = self
            .word()
            .is_some_and(|word| vocabulary::LAYOUT_EDGE_WORDS.contains(&word))
            && self
                .words
                .get(self.pos + 1)
                .is_some_and(|word| word == "of");
```

`fn reference(&mut self)` — replace `if matches!(word, "row" | "column")`:

```rust
        if vocabulary::LAYOUT_AXIS_WORDS.contains(&word) {
```

`fn direction(&mut self)` — keep the two-arm shape (the `above`/`below` arm allows an optional `left`/`right` before `of`, the `left`/`right` arm requires `of` immediately), and assert the words against the table so a table edit that drops one is caught in debug builds. Replace the whole body:

```rust
    fn direction(&mut self) -> Result<Option<std::ops::Range<usize>>, LayoutShapeError> {
        let start = self.pos;
        match self.word() {
            Some(word) if matches!(word, "above" | "below") => {
                debug_assert!(vocabulary::LAYOUT_DIRECTION_HEADS.contains(&word));
                self.pos += 1;
                if matches!(self.word(), Some("left") | Some("right")) {
                    self.pos += 1;
                    if !self.eat("of") {
                        return Err(self.error(
                            start,
                            self.pos,
                            UmlSyntaxKind::LayoutKeywordToken,
                            LayoutExpectation::Keyword {
                                after: self.words[start..self.pos].join(" "),
                                keyword: "of",
                            },
                        ));
                    }
                }
                Ok(Some(start..self.pos))
            }
            Some(word) if matches!(word, "left" | "right") => {
                debug_assert!(vocabulary::LAYOUT_DIRECTION_HEADS.contains(&word));
                self.pos += 1;
                if !self.eat("of") {
                    return Err(self.error(
                        start,
                        self.pos,
                        UmlSyntaxKind::LayoutKeywordToken,
                        LayoutExpectation::Keyword {
                            after: self.previous_word(),
                            keyword: "of",
                        },
                    ));
                }
                Ok(Some(start..self.pos))
            }
            _ => Ok(None),
        }
    }
```

The `debug_assert!` lines are the only behaviour change: `matches!` stays because the two arms treat their words differently, so a plain `contains` over one table would flatten a real grammar distinction.

Message verbs — at `:1959-1994` and `:2677`, replace each
`matches!(verb, "calls" | "returns" | "signals" | "creates" | "destroys")`
with `vocabulary::MESSAGE_VERBS.contains(&verb)`. Leave the narrower tests
(`matches!(verb, "replies" | "sends")`, `matches!(verb, "calls" | "returns" | "signals")`)
alone — those are grammar sub-cases, not the vocabulary.

- [ ] **Step 4: Run the gate**

Run: `cargo test --workspace && cargo clippy --workspace --all-targets --all-features -- -D warnings && cargo fmt --all -- --check`
Expected: all green. `crates/waml/tests/uml_diagram_syntax.rs` and `uml_behavior_syntax.rs` carry the dense layout/message parser coverage; a failure there means a word changed meaning.

- [ ] **Step 5: Commit**

```bash
git add crates/waml/src/uml/syntax/parser.rs crates/waml/tests/uml_vocabulary.rs
git commit -m "refactor(uml): parse layout and message keywords from the vocabulary

The parser's literal match arms were the second copy of the layout
keyword list. It now reads uml::vocabulary, so the parser, the formatter
and the coming completion providers cannot drift apart."
```

---

### Task 3: `ExpectedSyntax` names what is actually expected

**Files:**
- Modify: `crates/waml/src/uml/declared.rs:12-22` (five new variants, plus `expected_for_slot`)
- Modify: `crates/waml/src/uml/analysis.rs` — `declared_lifeline:2919-2977`, `declared_optional_message_token:3080-3095`, `declared_interaction_use:3248-3253`, `declared_binding:3220-3239`, `declared_gate:3137-3146`, `declared_slot:3282-3300`
- Test: `crates/waml/tests/uml_repair_actions.rs` (append)

**Interfaces:**
- Produces: `ExpectedSyntax::{Alias, InstanceName, SlotName, SlotValue, CallId}`; `pub fn expected_for_slot(slot: UmlSyntaxKind) -> ExpectedSyntax`.
- Consumes: `crate::uml::syntax::UmlSyntaxKind`.

This corrects a live defect: `declared_lifeline` reports a missing alias as `ExpectedSyntax::MessageTarget`, so every consumer of that field is told the wrong thing today. Fixing it here, before any completion code exists, keeps the diagnostic half and the completion half independent — as the spec requires.

- [ ] **Step 1: Write the failing test**

Append to `crates/waml/tests/uml_repair_actions.rs`:

```rust
use waml::uml::{declared::expected_for_slot, syntax::UmlSyntaxKind, ExpectedSyntax};

fn sequence(text: &str) -> PreparedCandidate {
    prepare_candidate(
        SourceBundle::try_from_pairs([
            ("class.md", text),
            ("a.md", "---\ntype: uml.Class\n---\n# A\n"),
        ])
        .unwrap(),
        None,
        5,
    )
    .unwrap()
}

#[test]
fn a_missing_lifeline_alias_is_reported_as_an_alias_not_a_message_target() {
    let candidate = sequence(
        "---\ntype: uml.Sequence\ntitle: S\n---\n# S\n\n## Lifelines\n\n- [A](./a.md) as\n",
    );
    let concept = candidate.uml().declared.concept("class").unwrap();
    assert!(
        matches!(
            concept.lifelines[0].alias,
            waml::uml::DeclaredField::Incomplete {
                expected: ExpectedSyntax::Alias,
                ..
            }
        ),
        "expected Alias, got something else"
    );
}

#[test]
fn a_missing_call_id_is_reported_as_a_call_id() {
    let candidate = sequence(
        "---\ntype: uml.Sequence\ntitle: S\n---\n# S\n\n## Lifelines\n\n- [A](./a.md) as A\n\n## Messages\n\n- A calls A `x()` as\n",
    );
    let concept = candidate.uml().declared.concept("class").unwrap();
    assert!(matches!(
        concept.messages[0].call_id,
        waml::uml::DeclaredField::Incomplete {
            expected: ExpectedSyntax::CallId,
            ..
        }
    ));
}

#[test]
fn the_slot_kind_table_is_total_over_the_slots_it_names() {
    assert_eq!(
        expected_for_slot(UmlSyntaxKind::LifelineAlias),
        ExpectedSyntax::Alias
    );
    assert_eq!(
        expected_for_slot(UmlSyntaxKind::InteractionUseAlias),
        ExpectedSyntax::Alias
    );
    assert_eq!(
        expected_for_slot(UmlSyntaxKind::MessageCallId),
        ExpectedSyntax::CallId
    );
    assert_eq!(
        expected_for_slot(UmlSyntaxKind::MessageReturnCall),
        ExpectedSyntax::CallId
    );
    assert_eq!(
        expected_for_slot(UmlSyntaxKind::MessageTarget),
        ExpectedSyntax::MessageTarget
    );
    assert_eq!(
        expected_for_slot(UmlSyntaxKind::FlowTarget),
        ExpectedSyntax::FlowTarget
    );
    // An unlisted slot keeps the historical catch-all rather than panicking:
    // a new slot kind must not turn a half-typed document into a crash.
    assert_eq!(
        expected_for_slot(UmlSyntaxKind::Root),
        ExpectedSyntax::MessageTarget
    );
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p waml --test uml_repair_actions`
Expected: FAIL — `no variant named Alias`.

- [ ] **Step 3: Add the variants and the slot-kind table**

In `crates/waml/src/uml/declared.rs`, extend the enum and add the table:

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExpectedSyntax {
    ColonToken,
    TypeReference,
    ValidMultiplicity,
    LinkTarget,
    RelationshipTarget,
    LayoutOperand,
    FlowTarget,
    MessageTarget,
    /// A handle the author invents: a lifeline or interaction-use alias.
    Alias,
    /// The name of an inline instance.
    InstanceName,
    /// The name half of a `## Slots` entry.
    SlotName,
    /// The value half of a `## Slots` entry.
    SlotValue,
    /// A message call id, declared after `as` or referenced after `for`.
    CallId,
}

/// The one table mapping a typed slot to what the grammar expects in it.
/// The analysis reads it to label an `Incomplete` field and `uml::complete`
/// reads the same slot kinds to pick a candidate provider -- neither reads the
/// other. An unlisted slot falls back to `MessageTarget`, which is what every
/// unlabelled site reported before this table existed, so adding a slot kind
/// can never turn an unfinished document into a panic.
pub fn expected_for_slot(slot: super::syntax::UmlSyntaxKind) -> ExpectedSyntax {
    use super::syntax::UmlSyntaxKind as K;
    match slot {
        K::LifelineAlias | K::InteractionUseAlias => ExpectedSyntax::Alias,
        K::MessageCallId | K::MessageReturnCall => ExpectedSyntax::CallId,
        K::InlineInstance => ExpectedSyntax::InstanceName,
        K::Slot => ExpectedSyntax::SlotName,
        K::FlowTarget => ExpectedSyntax::FlowTarget,
        K::TypeReference => ExpectedSyntax::TypeReference,
        K::Multiplicity => ExpectedSyntax::ValidMultiplicity,
        K::Link => ExpectedSyntax::LinkTarget,
        _ => ExpectedSyntax::MessageTarget,
    }
}
```

Export it: in `crates/waml/src/uml.rs`, add `expected_for_slot` to the `pub use declared::{...}` list.

- [ ] **Step 4: Route the analysis call sites through the table**

In `crates/waml/src/uml/analysis.rs`:

`declared_lifeline` — replace the hard-coded `MessageTarget` in the alias branch:

```rust
                    if as_present {
                        crate::uml::DeclaredField::Incomplete {
                            syntax: slot,
                            expected: crate::uml::declared::expected_for_slot(
                                syntax::UmlSyntaxKind::LifelineAlias,
                            ),
                        }
                    } else {
                        crate::uml::DeclaredField::Absent
                    }
```

`declared_optional_message_token` — this is the single funnel for `MessageCallId`, `MessageReturnTarget` and `MessageReturnCall`, so deriving from `slot_kind` fixes all three at once:

```rust
    declared_required_token(
        node,
        slot_kind,
        token_kind,
        crate::uml::declared::expected_for_slot(slot_kind),
    )
```

`declared_interaction_use`, `declared_binding`, `declared_gate` — replace each literal
`crate::uml::ExpectedSyntax::MessageTarget` argument with
`crate::uml::declared::expected_for_slot(syntax::UmlSyntaxKind::<the slot kind already passed one line above>)`.

`declared_slot` — replace the two literals so the two halves stop borrowing
`ColonToken`/`LinkTarget`:

```rust
    let name = field(syntax.name_token(), crate::uml::ExpectedSyntax::SlotName);
```
```rust
    let mut value = field(syntax.value_token(), crate::uml::ExpectedSyntax::SlotValue);
```

Leave the `ColonToken` expectation on the *missing colon* branch alone — a
missing `:` genuinely expects a colon.

- [ ] **Step 5: Run test to verify it passes**

Run: `cargo test -p waml --test uml_repair_actions`
Expected: PASS

- [ ] **Step 6: Run the gate**

Run: `cargo test --workspace && cargo clippy --workspace --all-targets --all-features -- -D warnings && cargo fmt --all -- --check`
Expected: green. Watch `crates/waml/tests/semantic_diagnostics.rs` and `uml_lowering_order.rs`: if either asserted the old `MessageTarget` label, update the assertion to the new variant and say why in the commit body — do not widen the table to keep an old assertion passing.

- [ ] **Step 7: Commit**

```bash
git add crates/waml/src/uml/declared.rs crates/waml/src/uml.rs crates/waml/src/uml/analysis.rs crates/waml/tests/uml_repair_actions.rs
git commit -m "fix(uml): name what an incomplete slot actually expects

declared_lifeline reported a missing alias as ExpectedSyntax::MessageTarget,
so every consumer of that field was told the wrong thing. ExpectedSyntax
gains Alias, InstanceName, SlotName, SlotValue and CallId, and the analysis
derives the label from one slot-kind table instead of repeating a literal
at each call site."
```

---

### Task 4: The locator

**Files:**
- Create: `crates/waml/src/uml/complete.rs`
- Modify: `crates/waml/src/uml.rs` (`mod complete;` plus re-exports)
- Modify: `crates/waml/src/analysis.rs:7` (re-export `TextRange`, `TextSize`)
- Test: `crates/waml/tests/uml_completion_locator.rs` (new)

**Interfaces:**
- Produces:
  - `pub struct Expectation { pub slot: UmlSyntaxKind, pub token: UmlSyntaxKind, pub node: SyntaxNode<UmlLanguage>, pub prefix: TextRange }`
  - `pub fn expectation_at(tree: &SyntaxTree<UmlLanguage>, offset: TextSize) -> Option<Expectation>`
  - `waml::analysis::{TextRange, TextSize}` re-exports
- Consumes: Task 3's slot kinds are unrelated; this task depends on nothing but the tree.

The locator carries no grammar table of its own. The parser already wrote one down as `UmlSyntaxKind`: a fixed slot holding a missing token at a real position is exactly the statement "an operand belongs here". `expectation_at` is `pub` and re-exported, so it is publicly reachable and `dead_code` does not fire before a caller exists.

**The rule, precisely.** For an offset `O` in a tree whose text is `source`:

1. `anchor` = `O` scanned left over ASCII space and tab.
2. **Mid-word.** If some non-missing token `t` has `t.trimmed_range().start() < O <= t.trimmed_range().end()`, then `token = t.kind()`, `node = t.parent()`, `prefix = t.trimmed_range()`.
3. **After the keyword.** Otherwise, if some missing (zero-width) token `t` has `t.range().start() == anchor`, then `token = t.kind()`, `node = t.parent()`, `prefix = TextRange::new(O, O)`.
4. Otherwise, `None`.
5. In either case the result is `Some` only when `token` is in `COMPLETION_TOKENS`. `slot = node.kind()` — the parser's fixed slot node directly above the token.

Rule 3 is why silence after a complete construct is free: `- Buyer calls Order |` has no missing token at the anchor, so it yields `None` without a special case. Prose, frontmatter and raw `## Operations` regions are `RawMarkdownToken` inside `MarkdownRegion`, which is not in `COMPLETION_TOKENS`, so they yield `None` too.

- [ ] **Step 1: Write the failing test**

Create `crates/waml/tests/uml_completion_locator.rs`:

```rust
//! The locator table. Each case is an authored document with a `|` marking the
//! cursor; the marker is stripped before parsing and its byte offset becomes
//! the query offset. Asserting slot and token kinds (never byte offsets) keeps
//! the table readable and keeps it from breaking on unrelated parser motion.

use waml::analysis::prepare_candidate;
use waml::source::{BundlePath, SourceBundle};
use waml::uml::{expectation_at, syntax::UmlSyntaxKind};

const SUPPORT: [(&str, &str); 2] = [
    ("a.md", "---\ntype: uml.Class\ntitle: A\n---\n# A\n"),
    ("b.md", "---\ntype: uml.Class\ntitle: B\n---\n# B\n"),
];

/// Parse `marked` with the `|` removed and return the locator result at the
/// marker, as `(slot, token, prefix_text)`.
fn locate(marked: &str) -> Option<(UmlSyntaxKind, UmlSyntaxKind, String)> {
    let offset = marked.find('|').expect("the case must place a cursor");
    let text = marked.replacen('|', "", 1);
    let mut pairs = vec![("doc.md", text.as_str())];
    pairs.extend(SUPPORT);
    let bundle = SourceBundle::try_from_pairs(pairs).unwrap();
    let candidate = prepare_candidate(bundle, None, 1).unwrap();
    let id = candidate
        .okf()
        .catalog
        .id_for_path(&BundlePath::parse("doc.md").unwrap())
        .unwrap();
    let snapshot = candidate.uml().syntax.document(id).unwrap();
    let offset = waml::analysis::TextSize::try_from_usize(offset).unwrap();
    expectation_at(snapshot.syntax(), offset).map(|expectation| {
        let prefix = text[expectation.prefix.start().to_usize()..expectation.prefix.end().to_usize()]
            .to_owned();
        (expectation.slot, expectation.token, prefix)
    })
}

fn sequence(body: &str) -> String {
    format!("---\ntype: uml.Sequence\ntitle: S\n---\n# S\n\n{body}")
}

#[test]
fn after_as_the_cursor_sits_in_the_empty_lifeline_alias_slot() {
    let located = locate(&sequence("## Lifelines\n\n- [A](./a.md) as |\n"));
    let (slot, token, prefix) = located.expect("an alias is expected after `as`");
    assert_eq!(slot, UmlSyntaxKind::LifelineAlias);
    assert_eq!(token, UmlSyntaxKind::AliasToken);
    assert_eq!(prefix, "");
}

#[test]
fn after_as_with_no_trailing_space_the_slot_is_still_found() {
    let located = locate(&sequence("## Lifelines\n\n- [A](./a.md) as|\n"));
    assert_eq!(
        located.map(|(slot, _, _)| slot),
        Some(UmlSyntaxKind::LifelineAlias)
    );
}

#[test]
fn mid_word_the_prefix_covers_the_partially_typed_operand() {
    let located = locate(&sequence(
        "## Lifelines\n\n- [A](./a.md) as A\n- [B](./b.md) as B\n\n## Messages\n\n- A calls B|\n",
    ));
    let (slot, token, prefix) = located.expect("a target is being typed");
    assert_eq!(slot, UmlSyntaxKind::MessageTarget);
    assert_eq!(token, UmlSyntaxKind::TargetToken);
    assert_eq!(prefix, "B");
}

#[test]
fn after_a_verb_the_cursor_sits_in_the_empty_target_slot() {
    let located = locate(&sequence(
        "## Lifelines\n\n- [A](./a.md) as A\n\n## Messages\n\n- A calls |\n",
    ));
    assert_eq!(
        located.map(|(slot, _, _)| slot),
        Some(UmlSyntaxKind::MessageTarget)
    );
}

#[test]
fn mid_verb_the_cursor_sits_in_the_verb_slot() {
    let located = locate(&sequence(
        "## Lifelines\n\n- [A](./a.md) as A\n\n## Messages\n\n- A cal|\n",
    ));
    let (slot, token, prefix) = located.expect("a verb is being typed");
    assert_eq!(slot, UmlSyntaxKind::MessageVerb);
    assert_eq!(token, UmlSyntaxKind::VerbToken);
    assert_eq!(prefix, "cal");
}

#[test]
fn silence_after_a_complete_construct_in_prose_and_in_a_raw_region() {
    // After a complete message: the operand is finished, nothing is expected.
    assert_eq!(
        locate(&sequence(
            "## Lifelines\n\n- [A](./a.md) as A\n- [B](./b.md) as B\n\n## Messages\n\n- A calls B |\n"
        )),
        None
    );
    // Inside prose.
    assert_eq!(locate(&sequence("Some prose about| the flow.\n")), None);
    // Inside a raw Markdown region that is not a WAML section.
    assert_eq!(
        locate(&sequence("## Operations\n\n- anything at all| here\n")),
        None
    );
    // Inside frontmatter.
    assert_eq!(
        locate("---\ntype: uml.Seq|uence\ntitle: S\n---\n# S\n"),
        None
    );
}

#[test]
fn an_offset_past_the_end_of_the_document_is_silence_not_an_error() {
    let bundle = SourceBundle::try_from_pairs([("doc.md", "# X\n")]).unwrap();
    let candidate = prepare_candidate(bundle, None, 1).unwrap();
    let id = candidate
        .okf()
        .catalog
        .id_for_path(&BundlePath::parse("doc.md").unwrap())
        .unwrap();
    let snapshot = candidate.uml().syntax.document(id).unwrap();
    let past_end = waml::analysis::TextSize::try_from_usize(10_000).unwrap();
    assert!(expectation_at(snapshot.syntax(), past_end).is_none());
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p waml --test uml_completion_locator`
Expected: FAIL — `cannot find function expectation_at`.

- [ ] **Step 3: Re-export the text types**

In `crates/waml/src/analysis.rs`, extend line 7:

```rust
pub use waml_syntax::{DocumentRevision, TextRange, TextSize};
```

(`waml::action::TextEdit::range` is already a `waml_syntax::TextRange` with no way for a consumer to name it. This re-export closes that hole as well as serving the locator.)

- [ ] **Step 4: Write the locator**

Create `crates/waml/src/uml/complete.rs`:

```rust
//! Completion suggestions at the positions where the WAML grammar expects an
//! operand. See `docs/superpowers/specs/2026-08-10-completion-suggestions-design.md`.
//!
//! The module has two halves that do not know about each other: a locator
//! answering *where am I*, and candidate providers answering *what goes here*.
//! Neither reads diagnostics; both read the parser's fixed slot kinds.

use waml_syntax::{SyntaxElement, SyntaxNode, SyntaxToken, SyntaxTree, TextRange, TextSize};

use super::syntax::{UmlLanguage, UmlSyntaxKind};

/// Token kinds that name an operand the author supplies. A token outside this
/// table -- prose, a newline, a bullet, end-of-file -- is never a completion
/// position, which is how silence in prose and after a complete construct
/// falls out with no special case.
const COMPLETION_TOKENS: &[UmlSyntaxKind] = &[
    UmlSyntaxKind::AliasToken,
    UmlSyntaxKind::SourceToken,
    UmlSyntaxKind::VerbToken,
    UmlSyntaxKind::TargetToken,
    UmlSyntaxKind::CallIdToken,
    UmlSyntaxKind::ReturnTargetToken,
    UmlSyntaxKind::ReturnCallToken,
    UmlSyntaxKind::LocalToken,
    UmlSyntaxKind::IdentityToken,
    UmlSyntaxKind::NodeKindToken,
    UmlSyntaxKind::FragmentKindToken,
    UmlSyntaxKind::RelationshipKindToken,
    UmlSyntaxKind::LinkTargetToken,
    UmlSyntaxKind::TypeToken,
    UmlSyntaxKind::LayoutWordToken,
    UmlSyntaxKind::LayoutKeywordToken,
    UmlSyntaxKind::IdentifierToken,
];

/// Where the cursor is, in the parser's own terms.
pub struct Expectation {
    /// The fixed slot node directly above the operand token.
    pub slot: UmlSyntaxKind,
    /// The operand token itself: missing when nothing was authored, present
    /// when the author is part-way through a word.
    pub token: UmlSyntaxKind,
    /// The slot node, for providers that need to look at its siblings.
    pub node: SyntaxNode<UmlLanguage>,
    /// The range a candidate replaces. Empty when nothing was authored.
    pub prefix: TextRange,
}

/// Find the token to the left of `offset`, skipping trivia, and report the slot
/// it belongs to. `None` -- after a complete construct, in prose, in a raw
/// Markdown region, past the end of the document -- is a normal result.
pub fn expectation_at(
    tree: &SyntaxTree<UmlLanguage>,
    offset: TextSize,
) -> Option<Expectation> {
    let source = tree.write_to_string();
    if offset.to_usize() > source.len() {
        return None;
    }
    let mut anchor = offset.to_usize();
    while anchor > 0 && matches!(source.as_bytes()[anchor - 1], b' ' | b'\t') {
        anchor -= 1;
    }
    let mut tokens = Vec::new();
    collect_tokens(&tree.root(), &mut tokens);

    // Mid-word: the author is part-way through an operand.
    let typed = tokens.iter().find(|token| {
        !token.flags().is_missing()
            && token.trimmed_range().start().to_usize() < offset.to_usize()
            && offset.to_usize() <= token.trimmed_range().end().to_usize()
    });
    if let Some(token) = typed {
        return expectation(token.clone(), token.trimmed_range());
    }

    // After the keyword: a zero-width missing token marks the empty slot.
    let missing = tokens.iter().find(|token| {
        token.flags().is_missing() && token.range().start().to_usize() == anchor
    })?;
    expectation(missing.clone(), TextRange::new(offset, offset).ok()?)
}

fn expectation(token: SyntaxToken<UmlLanguage>, prefix: TextRange) -> Option<Expectation> {
    if !COMPLETION_TOKENS.contains(&token.kind()) {
        return None;
    }
    let node = token.parent()?;
    Some(Expectation {
        slot: node.kind(),
        token: token.kind(),
        node,
        prefix,
    })
}

fn collect_tokens(node: &SyntaxNode<UmlLanguage>, out: &mut Vec<SyntaxToken<UmlLanguage>>) {
    for element in node.children() {
        match element {
            SyntaxElement::Token(token) => out.push(token),
            SyntaxElement::Node(child) => collect_tokens(&child, out),
        }
    }
}
```

- [ ] **Step 5: Wire the module**

In `crates/waml/src/uml.rs` add `mod complete;` (alphabetically, before `pub mod declared;`) and the re-export:

```rust
pub use complete::{expectation_at, Expectation};
```

- [ ] **Step 6: Run the locator table**

Run: `cargo test -p waml --test uml_completion_locator`
Expected: PASS.

**If a case returns `None` where the table expects a slot**, the parser does not emit a fixed missing token for that construct. **Do not change the parser in this task.** Change that one assertion to `assert_eq!(located, None)` and put a `// KNOWN GAP: <construct>; the parser emits recovery here, not a fixed slot.` comment above it, then note the gap in the commit body. Later provider tasks read this file to learn which positions are reachable.

- [ ] **Step 7: Run the gate**

Run: `cargo test --workspace && cargo clippy --workspace --all-targets --all-features -- -D warnings && cargo fmt --all -- --check`
Expected: green.

- [ ] **Step 8: Commit**

```bash
git add crates/waml/src/uml/complete.rs crates/waml/src/uml.rs crates/waml/src/analysis.rs crates/waml/tests/uml_completion_locator.rs
git commit -m "feat(uml): locate the expected operand slot at an offset

expectation_at finds the token left of the cursor and reports the fixed
slot it belongs to. It introduces no grammar table: the parser already
wrote one down as UmlSyntaxKind, and a fixed slot holding a zero-width
missing token at a real position is exactly the statement that an operand
belongs there."
```

---

### Task 5: `Completion`, the entry point, and `fixed_vocabulary`

**Files:**
- Modify: `crates/waml/src/uml/complete.rs`
- Modify: `crates/waml/src/uml.rs` (re-export `completions`, `Completion`, `CompletionKind`)
- Test: `crates/waml/tests/uml_completions.rs` (new)

**Interfaces:**
- Consumes: `expectation_at`, `Expectation` (Task 4); `uml::vocabulary` (Task 1); `uml::ActionContext` (`crates/waml/src/uml/format.rs:12`).
- Produces:
  - `pub struct Completion { pub label: Arc<str>, pub insert: Arc<str>, pub kind: CompletionKind, pub detail: Option<Arc<str>>, pub replace: TextRange }`
  - `pub enum CompletionKind { Keyword, Reference, Field, Value, Link, Name }`
  - `pub fn completions(context: ActionContext<'_>, document: DocumentId, offset: TextSize) -> Result<Vec<Completion>, ActionError>`

`Completion` carries `replace` in addition to the spec's four fields: the LSP consumer needs the range to build a `TextEdit`, and the spec requires that "the returned range covers it so a client replaces rather than appends". It is the `Expectation`'s `prefix`, copied onto each candidate so the entry point can stay a plain `Vec`.

- [ ] **Step 1: Write the failing test**

Create `crates/waml/tests/uml_completions.rs`:

```rust
use waml::analysis::{prepare_candidate, DocumentId, PreparedCandidate, TextSize};
use waml::source::{BundlePath, SourceBundle};
use waml::uml::{completions, ActionContext, CompletionKind};

const SUPPORT: [(&str, &str); 2] = [
    ("a.md", "---\ntype: uml.Class\ntitle: A\n---\n# A\n"),
    ("b.md", "---\ntype: uml.Class\ntitle: B\n---\n# B\n"),
];

fn prepared(text: &str, revision: u64) -> PreparedCandidate {
    let mut pairs = vec![("doc.md", text)];
    pairs.extend(SUPPORT);
    prepare_candidate(SourceBundle::try_from_pairs(pairs).unwrap(), None, revision).unwrap()
}

fn document(candidate: &PreparedCandidate) -> DocumentId {
    candidate
        .okf()
        .catalog
        .id_for_path(&BundlePath::parse("doc.md").unwrap())
        .unwrap()
}

/// Complete `marked` at its `|`, returning `(label, kind)` pairs.
fn labels(marked: &str) -> Vec<(String, CompletionKind)> {
    let offset = marked.find('|').expect("the case must place a cursor");
    let text = marked.replacen('|', "", 1);
    let candidate = prepared(&text, 3);
    completions(
        ActionContext::from_prepared(&candidate).unwrap(),
        document(&candidate),
        TextSize::try_from_usize(offset).unwrap(),
    )
    .unwrap()
    .into_iter()
    .map(|completion| (completion.label.to_string(), completion.kind))
    .collect()
}

fn sequence(body: &str) -> String {
    format!("---\ntype: uml.Sequence\ntitle: S\n---\n# S\n\n{body}")
}

#[test]
fn an_empty_verb_slot_offers_every_message_verb() {
    let offered = labels(&sequence(
        "## Lifelines\n\n- [A](./a.md) as A\n\n## Messages\n\n- A |\n",
    ));
    let mut words = offered
        .iter()
        .filter(|(_, kind)| *kind == CompletionKind::Keyword)
        .map(|(label, _)| label.as_str())
        .collect::<Vec<_>>();
    words.sort_unstable();
    assert_eq!(words, ["calls", "creates", "destroys", "returns", "signals"]);
}

#[test]
fn a_partially_typed_verb_filters_on_the_prefix_and_replaces_it() {
    let marked = sequence("## Lifelines\n\n- [A](./a.md) as A\n\n## Messages\n\n- A cr|\n");
    let offset = marked.find('|').unwrap();
    let text = marked.replacen('|', "", 1);
    let candidate = prepared(&text, 3);
    let offered = completions(
        ActionContext::from_prepared(&candidate).unwrap(),
        document(&candidate),
        TextSize::try_from_usize(offset).unwrap(),
    )
    .unwrap();
    assert_eq!(
        offered
            .iter()
            .map(|completion| completion.label.to_string())
            .collect::<Vec<_>>(),
        ["creates"]
    );
    let replace = offered[0].replace;
    assert_eq!(&text[replace.start().to_usize()..replace.end().to_usize()], "cr");
}

#[test]
fn a_fragment_kind_slot_offers_every_fragment_keyword() {
    let offered = labels(&sequence(
        "## Lifelines\n\n- [A](./a.md) as A\n\n## Messages\n\n- |\n",
    ));
    // The bullet with nothing after it is a fragment-or-message position; the
    // fragment keywords must be present whatever else is.
    for keyword in ["alt", "opt", "loop", "par"] {
        assert!(
            offered.iter().any(|(label, _)| label == keyword),
            "{keyword} missing from {offered:?}"
        );
    }
}

#[test]
fn a_relationship_kind_slot_offers_every_relationship_keyword() {
    let offered = labels(
        "---\ntype: uml.Class\ntitle: C\n---\n# C\n\n## Relationships\n\n- | [A](./a.md)\n",
    );
    for keyword in ["associates", "composes", "instance of", "specializes"] {
        assert!(
            offered.iter().any(|(label, _)| label == keyword),
            "{keyword} missing from {offered:?}"
        );
    }
}

#[test]
fn nothing_is_offered_in_prose_or_after_a_complete_construct() {
    assert!(labels(&sequence("Some prose about| the flow.\n")).is_empty());
    assert!(labels(&sequence(
        "## Lifelines\n\n- [A](./a.md) as A\n- [B](./b.md) as B\n\n## Messages\n\n- A calls B |\n"
    ))
    .is_empty());
}

#[test]
fn an_offset_past_the_end_of_the_document_is_empty_not_an_error() {
    let candidate = prepared("---\ntype: uml.Class\n---\n# C\n", 3);
    assert_eq!(
        completions(
            ActionContext::from_prepared(&candidate).unwrap(),
            document(&candidate),
            TextSize::try_from_usize(9_999).unwrap(),
        )
        .unwrap(),
        Vec::new()
    );
}

#[test]
fn an_unknown_document_is_an_error() {
    let candidate = prepared("---\ntype: uml.Class\n---\n# C\n", 3);
    let other = prepare_candidate(
        SourceBundle::try_from_pairs([("other.md", "# Other\n")]).unwrap(),
        None,
        3,
    )
    .unwrap();
    let stranger = other
        .okf()
        .catalog
        .id_for_path(&BundlePath::parse("other.md").unwrap())
        .unwrap();
    assert!(matches!(
        completions(
            ActionContext::from_prepared(&candidate).unwrap(),
            stranger,
            TextSize::try_from_usize(0).unwrap(),
        ),
        Err(waml::action::ActionError::UnknownDocument { .. })
    ));
}

#[test]
fn a_stale_session_revision_is_an_error_before_any_candidate_is_computed() {
    let candidate = prepared("---\ntype: uml.Class\n---\n# C\n", 3);
    assert!(matches!(
        ActionContext::new(candidate.okf(), candidate.uml(), 4),
        Err(waml::action::ActionError::MismatchedAnalysisRevision { .. })
    ));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p waml --test uml_completions`
Expected: FAIL — `cannot find function completions`.

- [ ] **Step 3: Add the types, the entry point, and the first provider**

Append to `crates/waml/src/uml/complete.rs` (and extend the `use` block with
`std::sync::Arc`, `crate::action::ActionError`, `crate::analysis::DocumentId`,
`crate::uml::{vocabulary, ActionContext}`):

```rust
/// What family a candidate came from. Consumers map this onto their own icon
/// vocabulary; `waml` must not gain an LSP dependency, so nothing here names
/// an LSP type.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum CompletionKind {
    /// A closed grammar word.
    Keyword,
    /// A handle or id declared elsewhere in this document.
    Reference,
    /// A slot or attribute name from a classifier.
    Field,
    /// An enum member or other value.
    Value,
    /// A document from the catalog.
    Link,
    /// A name the author is inventing.
    Name,
}

/// One candidate. `insert` is what goes into the document; `label` is what the
/// client shows; `replace` is the range the insertion covers, so a client
/// replaces a half-typed word rather than appending to it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Completion {
    pub label: Arc<str>,
    pub insert: Arc<str>,
    pub kind: CompletionKind,
    pub detail: Option<Arc<str>>,
    pub replace: TextRange,
}

/// Candidates for `offset` in `document`.
///
/// Mirrors `repair_actions`: an unknown document is an error, a catalog or
/// revision mismatch is an error (`ActionContext::new` already made that
/// check), and everything else -- an offset past the end, an incomplete tree,
/// a position in prose -- is an empty list. An unfinished document is the
/// expected input, not a failure.
pub fn completions(
    context: ActionContext<'_>,
    document: DocumentId,
    offset: TextSize,
) -> Result<Vec<Completion>, ActionError> {
    let version = context
        .okf()
        .catalog
        .document(document)
        .ok_or(ActionError::UnknownDocument { document })?;
    let Some(snapshot) = context.uml().syntax.document(document) else {
        return Ok(Vec::new());
    };
    if !Arc::ptr_eq(version, snapshot.document()) {
        return Err(ActionError::MismatchedCatalog);
    }
    let source = snapshot.syntax().write_to_string();
    let Some(expectation) = expectation_at(snapshot.syntax(), offset) else {
        return Ok(Vec::new());
    };
    let mut candidates = fixed_vocabulary(&expectation);
    // Later slices append further providers here; each is selected on the slot
    // and token kinds alone, so adding a family is a new function and a match
    // arm and the locator never changes.
    let prefix = source
        .get(expectation.prefix.start().to_usize()..expectation.prefix.end().to_usize())
        .unwrap_or("")
        .to_ascii_lowercase();
    candidates.retain(|candidate| candidate.label.to_ascii_lowercase().starts_with(&prefix));
    candidates.sort_by(|left, right| {
        (left.kind, left.label.as_ref()).cmp(&(right.kind, right.label.as_ref()))
    });
    candidates.dedup_by(|left, right| left.kind == right.kind && left.label == right.label);
    Ok(candidates)
}

fn keyword(word: &str, replace: TextRange, detail: &str) -> Completion {
    Completion {
        label: Arc::from(word),
        insert: Arc::from(word),
        kind: CompletionKind::Keyword,
        detail: Some(Arc::from(detail)),
        replace,
    }
}

/// Closed grammar words: message verbs, relationship kinds, flow node kinds,
/// fragment kinds, layout directions and hints. Requires no analysis. Every
/// word comes from `uml::vocabulary`; none is retyped here.
fn fixed_vocabulary(expectation: &Expectation) -> Vec<Completion> {
    let replace = expectation.prefix;
    match expectation.token {
        UmlSyntaxKind::VerbToken => vocabulary::MESSAGE_VERBS
            .iter()
            .map(|word| keyword(word, replace, "message verb"))
            .collect(),
        UmlSyntaxKind::RelationshipKindToken => vocabulary::relationship_keywords()
            .map(|word| keyword(word, replace, "relationship kind"))
            .collect(),
        UmlSyntaxKind::NodeKindToken => vocabulary::flow_node_keywords()
            .map(|word| keyword(word, replace, "flow node kind"))
            .collect(),
        UmlSyntaxKind::FragmentKindToken => vocabulary::fragment_keywords()
            .map(|word| keyword(word, replace, "combined fragment"))
            .collect(),
        UmlSyntaxKind::LayoutWordToken | UmlSyntaxKind::LayoutKeywordToken => {
            vocabulary::LAYOUT_DIRECTION_PHRASES
                .iter()
                .map(|word| keyword(word, replace, "layout direction"))
                .chain(
                    vocabulary::LAYOUT_HINT_PHRASES
                        .iter()
                        .map(|word| keyword(word, replace, "layout hint")),
                )
                .collect()
        }
        _ => Vec::new(),
    }
}
```

- [ ] **Step 4: Re-export**

In `crates/waml/src/uml.rs`, extend the completion re-export:

```rust
pub use complete::{completions, Completion, CompletionKind, Expectation};
```

(`expectation_at` stays exported; the locator table test names it.)

- [ ] **Step 5: Run test to verify it passes**

Run: `cargo test -p waml --test uml_completions`
Expected: PASS.

If `a_fragment_kind_slot_offers_every_fragment_keyword` or
`a_relationship_kind_slot_offers_every_relationship_keyword` fails with an
empty list, check `crates/waml/tests/uml_completion_locator.rs` for a
`KNOWN GAP` note on that construct: the parser emits recovery there rather
than a fixed slot. If so, delete that test and record the gap in the commit
body. Do not change the parser to make it pass — that is a separate decision.

- [ ] **Step 6: Run the gate**

Run: `cargo test --workspace && cargo clippy --workspace --all-targets --all-features -- -D warnings && cargo fmt --all -- --check`
Expected: green.

- [ ] **Step 7: Commit**

```bash
git add crates/waml/src/uml/complete.rs crates/waml/src/uml.rs crates/waml/tests/uml_completions.rs
git commit -m "feat(uml): offer the closed grammar vocabulary at empty slots

completions() mirrors repair_actions' entry contract: an unknown document
or a catalog/revision mismatch is an error, everything else is an empty
list. The first provider offers message verbs, relationship kinds, flow
node kinds, fragment kinds and layout words, all read from
uml::vocabulary rather than retyped."
```

---

### Task 6: Serve completions over the LSP

**Files:**
- Modify: `crates/waml-cli/src/lsp/query.rs` (add `completion`, beside `definition:148`)
- Modify: `crates/waml-cli/src/lsp/server.rs` (`server_capabilities:61`, a `completion` handler, and the test assertion at `:388`)

**Interfaces:**
- Consumes: `waml::uml::{completions, ActionContext, Completion, CompletionKind}` (Task 5); `crate::lsp::map::{from_lsp_position, to_lsp_range}`.
- Produces: `LspAnalysisState::completion(&self, physical: &Path, position: lsp::Position) -> Option<Vec<lsp::CompletionItem>>`.

**`waml-cli` is a bin-only crate — `pub` does not exempt an item from `dead_code`.** The query function and its caller must land in the same commit; that is why this task covers both files.

- [ ] **Step 1: Write the failing test**

In `crates/waml-cli/src/lsp/server.rs`, update the existing capability assertion at line 388. Replace:

```rust
        assert!(capabilities.completion_provider.is_none());
```

with:

```rust
        // Reversed by docs/superpowers/specs/2026-08-10-completion-suggestions-design.md.
        // The previous assertion recorded a deliberate decision not to offer
        // completions; the spec supersedes it, so this is updated rather than
        // deleted.
        let completion = capabilities
            .completion_provider
            .expect("completion is advertised");
        assert_eq!(
            completion.trigger_characters,
            Some(vec![" ".to_string(), "(".to_string()])
        );
        assert_eq!(completion.resolve_provider, Some(false));
```

And add a new test to the same `mod tests`:

```rust
    #[test]
    fn completion_offers_message_verbs_at_an_empty_verb_slot() {
        let physical = PathBuf::from("C:/outside/seq.md");
        let text = concat!(
            "---\ntype: uml.Sequence\ntitle: S\n---\n# S\n\n",
            "## Lifelines\n\n- [A](./a.md) as A\n\n## Messages\n\n- A \n"
        );
        let state = LspAnalysisState::empty()
            .unwrap()
            .open(physical.clone(), 1, text.into())
            .unwrap();
        // The cursor sits at the end of the "- A " line.
        let line = text.lines().count() as u32 - 1;
        let items = state
            .completion(&physical, Position::new(line, 4))
            .expect("completion returns a list");
        let labels = items
            .iter()
            .map(|item| item.label.as_str())
            .collect::<Vec<_>>();
        assert!(labels.contains(&"calls"), "{labels:?}");
        assert!(labels.contains(&"returns"), "{labels:?}");
        assert!(items
            .iter()
            .all(|item| item.text_edit.is_some() || item.insert_text.is_some()));
    }

    #[test]
    fn completion_in_prose_is_an_empty_list_not_an_absent_response() {
        let physical = PathBuf::from("C:/outside/prose.md");
        let state = LspAnalysisState::empty()
            .unwrap()
            .open(physical.clone(), 1, "# Title\n\nJust prose here.\n".into())
            .unwrap();
        assert_eq!(
            state.completion(&physical, Position::new(2, 5)),
            Some(Vec::new())
        );
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p waml-cli --bin waml lsp::server`
Expected: FAIL — `no method named completion`.

- [ ] **Step 3: Add the query**

In `crates/waml-cli/src/lsp/query.rs`, extend the imports:

```rust
use waml::{
    analysis::{DocumentId, DocumentVersion, MarkdownTokenRole, TextSize, WamlCodeRole},
    okf,
    source::BundlePath,
    uml::{completions, ActionContext, CompletionKind},
};
```

and add to `impl LspAnalysisState`, right after `definition`:

```rust
    /// Candidates at `position`. `None` means "not a document I know"; an
    /// empty `Vec` means "nothing is expected here", which is a normal result
    /// and must stay distinguishable from the former.
    pub fn completion(
        &self,
        physical: &Path,
        position: lsp::Position,
    ) -> Option<Vec<lsp::CompletionItem>> {
        let (_, document_id, document) = self.query_document(physical)?;
        let offset = from_lsp_position(position, document).ok()?;
        let offset = TextSize::try_from_usize(offset).ok()?;
        let context = ActionContext::new(&self.okf, &self.uml, self.revision).ok()?;
        let candidates = completions(context, document_id, offset).ok()?;
        let items = candidates
            .into_iter()
            .filter_map(|candidate| {
                let range = to_lsp_range(
                    candidate.replace.start().to_usize()..candidate.replace.end().to_usize(),
                    document,
                )
                .ok()?;
                Some(lsp::CompletionItem {
                    label: candidate.label.to_string(),
                    kind: Some(completion_item_kind(candidate.kind)),
                    detail: candidate.detail.map(|detail| detail.to_string()),
                    text_edit: Some(lsp::CompletionTextEdit::Edit(lsp::TextEdit {
                        range,
                        new_text: candidate.insert.to_string(),
                    })),
                    ..Default::default()
                })
            })
            .collect();
        Some(items)
    }
```

and a free function beside `heading_slug`:

```rust
fn completion_item_kind(kind: CompletionKind) -> lsp::CompletionItemKind {
    match kind {
        CompletionKind::Keyword => lsp::CompletionItemKind::KEYWORD,
        CompletionKind::Reference => lsp::CompletionItemKind::REFERENCE,
        CompletionKind::Field => lsp::CompletionItemKind::FIELD,
        CompletionKind::Value => lsp::CompletionItemKind::ENUM_MEMBER,
        CompletionKind::Link => lsp::CompletionItemKind::FILE,
        CompletionKind::Name => lsp::CompletionItemKind::VARIABLE,
    }
}
```

Both `document` and `context` borrow `&self` immutably, so they coexist; the
range conversion happens after `completions` returns, and `candidates` owns its
data, so nothing borrows across the call.

- [ ] **Step 4: Advertise and handle it**

In `crates/waml-cli/src/lsp/server.rs`, add to `server_capabilities()` before `..Default::default()`:

```rust
        completion_provider: Some(CompletionOptions {
            resolve_provider: Some(false),
            // A space commits the previous word, which is exactly when an empty
            // operand slot appears; `(` opens a link target.
            trigger_characters: Some(vec![" ".to_string(), "(".to_string()]),
            ..Default::default()
        }),
```

and add the handler to `impl LanguageServer for Backend`, after `goto_definition`:

```rust
    async fn completion(&self, params: CompletionParams) -> Result<Option<CompletionResponse>> {
        let Some(physical) = params
            .text_document_position
            .text_document
            .uri
            .to_file_path()
            .map(|p| p.into_owned())
        else {
            return Ok(None);
        };
        let position = params.text_document_position.position;
        Ok(self
            .current_query(|snapshot| snapshot.completion(&physical, position))
            .await
            .map(CompletionResponse::Array))
    }
```

- [ ] **Step 5: Run the tests**

Run: `cargo test -p waml-cli --bin waml`
Expected: PASS.

If `completion_offers_message_verbs_at_an_empty_verb_slot` returns an empty
list, print the offset the test computed and compare it against the document
text: `Position::new(line, 4)` must land immediately after `- A ` on the last
content line. Adjust the position, never the locator.

- [ ] **Step 6: Run the gate**

Run: `cargo test --workspace && cargo clippy --workspace --all-targets --all-features -- -D warnings && cargo fmt --all -- --check`
Then, from `editors/vscode`: `pnpm build && pnpm test && pnpm lint`
Expected: all green. The extension needs no change — it is a thin `LanguageClient` shell and picks the capability up from `initialize`.

- [ ] **Step 7: Commit**

```bash
git add crates/waml-cli/src/lsp/query.rs crates/waml-cli/src/lsp/server.rs
git commit -m "feat(lsp): serve textDocument/completion

The server advertises completion_provider and maps waml::uml::completions
onto CompletionItems with a replace-range TextEdit. The capability test
that asserted completion_provider.is_none() recorded a deliberate past
decision, so it is updated with a comment naming the spec that supersedes
it rather than deleted."
```

---

### Task 7: LSP completion round trip over stdio

**Files:**
- Modify: `crates/waml-cli/tests/lsp_e2e.rs`

**Interfaces:**
- Consumes: the capability and handler from Task 6.
- Produces: nothing. Tests only.

- [ ] **Step 1: Write the test**

Append to `crates/waml-cli/tests/lsp_e2e.rs`:

```rust
// Scenario: LSP-006
#[test]
fn completion_is_advertised_and_returns_items_over_stdio() {
    let exe = env!("CARGO_BIN_EXE_waml");
    let mut child = Command::new(exe)
        .args(["lsp", "--stdio"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn waml lsp");
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = child.stdout.take().unwrap();
    let (tx, rx) = mpsc::channel::<String>();
    let reader = std::thread::spawn(move || {
        let mut out = String::new();
        let mut buf = [0u8; 8192];
        loop {
            match stdout.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    out.push_str(&String::from_utf8_lossy(&buf[..n]));
                    if tx.send(out.clone()).is_err() {
                        break;
                    }
                }
            }
        }
    });
    let wait_for = |marker: &str| {
        let deadline = Instant::now() + Duration::from_secs(20);
        let mut out = String::new();
        while Instant::now() < deadline {
            match rx.recv_timeout(Duration::from_secs(20)) {
                Ok(latest) => {
                    out = latest;
                    if out.contains(marker) {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
        out
    };
    let send = |stdin: &mut std::process::ChildStdin, value: serde_json::Value| {
        stdin
            .write_all(frame(&value.to_string()).as_bytes())
            .unwrap();
        stdin.flush().unwrap();
    };

    send(
        &mut stdin,
        serde_json::json!({
            "jsonrpc": "2.0", "id": 1, "method": "initialize",
            "params": {"capabilities": {}}
        }),
    );
    let initialized = framed_json(&wait_for("\"id\":1"))
        .into_iter()
        .find(|value| value["id"] == 1)
        .unwrap();
    assert!(
        initialized["result"]["capabilities"]["completionProvider"].is_object(),
        "{initialized}"
    );

    send(
        &mut stdin,
        serde_json::json!({"jsonrpc": "2.0", "method": "initialized", "params": {}}),
    );
    let uri = "file:///C:/tmp/completion-seq.md";
    // Line 9 (0-based) is "- A ", so character 4 is the empty verb slot.
    let text = "---\ntype: uml.Sequence\ntitle: S\n---\n# S\n\n## Lifelines\n\n- [A](./a.md) as A\n\n## Messages\n\n- A \n";
    send(
        &mut stdin,
        serde_json::json!({
            "jsonrpc": "2.0", "method": "textDocument/didOpen",
            "params": {"textDocument": {
                "uri": uri, "languageId": "markdown", "version": 1, "text": text
            }}
        }),
    );
    let line = text.lines().count() as u32 - 1;
    send(
        &mut stdin,
        serde_json::json!({
            "jsonrpc": "2.0", "id": 2, "method": "textDocument/completion",
            "params": {"textDocument": {"uri": uri}, "position": {"line": line, "character": 4}}
        }),
    );
    let response = framed_json(&wait_for("\"id\":2"))
        .into_iter()
        .find(|value| value["id"] == 2)
        .unwrap();
    let labels = response["result"]
        .as_array()
        .unwrap_or_else(|| panic!("expected an array of items, got {response}"))
        .iter()
        .map(|item| item["label"].as_str().unwrap().to_owned())
        .collect::<Vec<_>>();
    assert!(labels.contains(&"calls".to_string()), "{labels:?}");

    let _ = child.kill();
    let _ = child.wait();
    drop(rx);
    let _ = reader.join();
}
```

- [ ] **Step 2: Run it**

Run: `cargo test -p waml-cli --test lsp_e2e completion_is_advertised`
Expected: PASS. If `line` is off, print `text.lines().enumerate()` and pick the
`- A ` line — do not weaken the assertion.

- [ ] **Step 3: Run the gate**

Run: `cargo test --workspace && cargo clippy --workspace --all-targets --all-features -- -D warnings && cargo fmt --all -- --check`
Expected: green.

- [ ] **Step 4: Commit**

```bash
git add crates/waml-cli/tests/lsp_e2e.rs
git commit -m "test(lsp): round-trip textDocument/completion over stdio

Drives the compiled server end to end: the capability appears in the
initialize result and a request at an empty verb slot returns the message
verbs. This is the point at which VS Code has working completions."
```

**Slice 2 is complete here.** Everything after this is additive; stopping now leaves a coherent, useful feature.

---

### Task 8: In-document lifeline handles at message endpoints

**Files:**
- Modify: `crates/waml/src/uml/complete.rs`
- Test: `crates/waml/tests/uml_completions.rs` (append)

**Interfaces:**
- Consumes: `ActionContext`, `Expectation` (Tasks 4–5); `context.uml().declared.concept(id)`; `crate::okf::id_of`.
- Produces: private `fn in_document_refs(expectation: &Expectation, concept: &DeclaredConcept) -> Vec<Completion>`, wired into `completions`. It needs no `ActionContext`: everything it offers is declared in this document.

`completions` does not read the declared concept yet; this task adds the lookup as an `if let`, so a document with no declared concept still returns the fixed vocabulary rather than an empty list.

- [ ] **Step 1: Write the failing test**

Append to `crates/waml/tests/uml_completions.rs`:

```rust
const SEQUENCE_BODY: &str = concat!(
    "## Lifelines\n\n",
    "- [A](./a.md) as buyer\n",
    "- [B](./b.md) as order\n\n",
    "## Gates\n\n",
    "- inbound\n\n",
    "## Messages\n\n",
);

#[test]
fn a_message_target_offers_every_declared_handle_plus_outside_and_gates() {
    let offered = labels(&sequence(&format!("{SEQUENCE_BODY}- buyer calls |\n")));
    let references = offered
        .iter()
        .filter(|(_, kind)| *kind == CompletionKind::Reference)
        .map(|(label, _)| label.as_str())
        .collect::<Vec<_>>();
    assert!(references.contains(&"buyer"), "{references:?}");
    assert!(references.contains(&"order"), "{references:?}");
    assert!(references.contains(&"outside"), "{references:?}");
    assert!(references.contains(&"@inbound"), "{references:?}");
}

#[test]
fn a_message_source_offers_the_same_handles() {
    let offered = labels(&sequence(&format!("{SEQUENCE_BODY}- |\n")));
    let references = offered
        .iter()
        .filter(|(_, kind)| *kind == CompletionKind::Reference)
        .map(|(label, _)| label.as_str())
        .collect::<Vec<_>>();
    assert!(references.contains(&"buyer"), "{references:?}");
    assert!(references.contains(&"order"), "{references:?}");
}

#[test]
fn a_handle_prefix_narrows_the_offered_handles() {
    let offered = labels(&sequence(&format!("{SEQUENCE_BODY}- buyer calls or|\n")));
    assert_eq!(
        offered
            .iter()
            .map(|(label, _)| label.as_str())
            .collect::<Vec<_>>(),
        ["order"]
    );
}

#[test]
fn a_use_gate_handle_is_offered_after_an_interaction_use() {
    let offered = labels(&sequence(concat!(
        "## Lifelines\n\n- [A](./a.md) as buyer\n\n",
        "## Messages\n\n- ref [B](./b.md) as inner\n- buyer calls |\n"
    )));
    assert!(
        offered.iter().any(|(label, _)| label == "inner@"),
        "the use alias must be offered as a gate prefix: {offered:?}"
    );
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p waml --test uml_completions`
Expected: FAIL — the reference labels are absent.

- [ ] **Step 3: Look the concept up in the entry point**

In `completions`, after the `expectation_at` line:

```rust
    let mut candidates = fixed_vocabulary(&expectation);
    let concept_id = crate::okf::id_of(version.path().as_str());
    if let Some(concept) = context.uml().declared.concept(&concept_id) {
        candidates.extend(in_document_refs(&expectation, concept));
    }
```

- [ ] **Step 4: Write the provider**

Append to `crates/waml/src/uml/complete.rs`:

```rust
use crate::uml::{DeclaredConcept, DeclaredField};

fn reference(value: &str, replace: TextRange, detail: &str) -> Completion {
    Completion {
        label: Arc::from(value),
        insert: Arc::from(value),
        kind: CompletionKind::Reference,
        detail: Some(Arc::from(detail)),
        replace,
    }
}

fn declared_name(field: &DeclaredField<UmlLanguage, String>) -> Option<&str> {
    match field {
        DeclaredField::Valid { value, .. } => Some(value.as_str()),
        _ => None,
    }
}

/// Names declared elsewhere in this document. Every candidate is a value the
/// diagnostic at this position accepts -- `UnknownLifelineHandle` for an
/// endpoint, and so on -- which Task 10 pins as a property test.
fn in_document_refs(expectation: &Expectation, concept: &DeclaredConcept) -> Vec<Completion> {
    let replace = expectation.prefix;
    match expectation.token {
        UmlSyntaxKind::SourceToken
        | UmlSyntaxKind::TargetToken
        | UmlSyntaxKind::ReturnTargetToken => {
            let mut out = Vec::new();
            for lifeline in concept.lifelines.iter() {
                if let Some(alias) = declared_name(&lifeline.alias) {
                    out.push(reference(alias, replace, "lifeline handle"));
                }
            }
            for gate in concept.gates.iter() {
                if let Some(name) = declared_name(&gate.name) {
                    out.push(reference(&format!("@{name}"), replace, "local gate"));
                }
            }
            for use_ in concept.interaction_uses.iter() {
                if let Some(alias) = declared_name(&use_.alias) {
                    out.push(reference(
                        &format!("{alias}@"),
                        replace,
                        "gate on an interaction use",
                    ));
                }
            }
            out.push(reference("outside", replace, "outside the frame"));
            out
        }
        _ => Vec::new(),
    }
}
```

- [ ] **Step 5: Run test to verify it passes**

Run: `cargo test -p waml --test uml_completions`
Expected: PASS.

- [ ] **Step 6: Run the gate**

Run: `cargo test --workspace && cargo clippy --workspace --all-targets --all-features -- -D warnings && cargo fmt --all -- --check`
Expected: green.

- [ ] **Step 7: Commit**

```bash
git add crates/waml/src/uml/complete.rs crates/waml/tests/uml_completions.rs
git commit -m "feat(uml): offer declared lifeline handles at message endpoints

Message source, target and return-target slots offer every declared
handle plus the outside and gate forms, all read from the declared model
rather than from the source text."
```

---

### Task 9: In-document call ids, flow targets, binding targets and layout refs

**Files:**
- Modify: `crates/waml/src/uml/complete.rs` (extend `in_document_refs`)
- Test: `crates/waml/tests/uml_completions.rs` (append)

**Interfaces:**
- Consumes: `in_document_refs` (Task 8); `DeclaredConcept::{messages, flow_nodes, lifelines, members, member_groups, interaction_uses}`.
- Produces: nothing new; extends the same match.

The `MessageCallId` / `MessageReturnCall` split is the evidence the whole approach rests on: `as` builds `MessageCallId`/`CallIdToken` (a declaration) and `for` builds `MessageReturnCall`/`ReturnCallToken` (a reference). Because the parser already distinguishes them, this provider needs no keyword test bolted on beside the slot kind.

- [ ] **Step 1: Write the failing test**

Append to `crates/waml/tests/uml_completions.rs`:

```rust
#[test]
fn for_offers_declared_call_ids_and_as_offers_none() {
    let body = concat!(
        "## Lifelines\n\n- [A](./a.md) as buyer\n- [B](./b.md) as order\n\n",
        "## Messages\n\n- buyer calls order `submit()` as submission\n"
    );
    let referenced = labels(&sequence(&format!(
        "{body}- order returns to buyer for |\n"
    )));
    assert!(
        referenced.iter().any(|(label, _)| label == "submission"),
        "a declared call id must be offered after `for`: {referenced:?}"
    );

    // `as` declares a call id; there is nothing in the document to reference,
    // so no Reference candidate is offered there.
    let declared = labels(&sequence(&format!(
        "{body}- buyer calls order `pay()` as |\n"
    )));
    assert!(
        !declared
            .iter()
            .any(|(label, kind)| label == "submission" && *kind == CompletionKind::Reference),
        "a declaration site must not offer existing ids: {declared:?}"
    );
}

#[test]
fn transitions_to_offers_declared_flow_node_identities() {
    let offered = labels(concat!(
        "---\ntype: uml.Activity\ntitle: F\n---\n# F\n\n",
        "## Nodes\n\n",
        "### Receive\n\n- transitions to Check\n\n",
        "### Check\n\n- transitions to |\n"
    ));
    let references = offered
        .iter()
        .filter(|(_, kind)| *kind == CompletionKind::Reference)
        .map(|(label, _)| label.as_str())
        .collect::<Vec<_>>();
    assert!(references.contains(&"Receive"), "{references:?}");
    assert!(references.contains(&"Check"), "{references:?}");
}

#[test]
fn bind_to_offers_the_used_interactions_handles() {
    let offered = labels(&sequence(concat!(
        "## Lifelines\n\n- [A](./a.md) as buyer\n\n",
        "## Messages\n\n- ref [B](./b.md) as inner\n  - bind buyer to |\n"
    )));
    // With no analysis of the used document's handles available, the provider
    // offers this document's handles, which is the accept set the diagnostic
    // uses for the local half of a binding.
    assert!(
        offered.iter().any(|(label, _)| label == "buyer"),
        "{offered:?}"
    );
}

#[test]
fn layout_offers_diagram_member_names() {
    let offered = labels(concat!(
        "---\ntype: Diagram\ntitle: D\nprofile: uml-domain\n---\n# D\n\n",
        "## Members\n\n- [A](./a.md)\n- [B](./b.md)\n\n",
        "## Layout\n\n- A above |\n"
    ));
    let references = offered
        .iter()
        .filter(|(_, kind)| *kind == CompletionKind::Reference)
        .map(|(label, _)| label.as_str())
        .collect::<Vec<_>>();
    assert!(references.contains(&"B"), "{references:?}");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p waml --test uml_completions`
Expected: FAIL.

- [ ] **Step 3: Extend the provider**

Add arms to `in_document_refs`:

```rust
        // `for` references an id the author already declared with `as`. The
        // parser gives the two positions different slot kinds, so no keyword
        // test is needed here.
        UmlSyntaxKind::ReturnCallToken => concept
            .messages
            .iter()
            .filter_map(|message| declared_name(&message.call_id))
            .map(|id| reference(id, replace, "declared call id"))
            .collect(),
        UmlSyntaxKind::IdentityToken => concept
            .flow_nodes
            .iter()
            .filter_map(|node| declared_name(&node.identity))
            .map(|identity| reference(identity, replace, "flow node"))
            .collect(),
        UmlSyntaxKind::LocalToken => concept
            .lifelines
            .iter()
            .filter_map(|lifeline| declared_name(&lifeline.alias))
            .map(|alias| reference(alias, replace, "lifeline handle"))
            .collect(),
        UmlSyntaxKind::LayoutWordToken => {
            let mut out = Vec::new();
            let mut push_member = |target: &str| {
                out.push(reference(target, replace, "diagram member"));
            };
            for member in concept.members.iter() {
                if let Some(target) = declared_name(&member.target) {
                    push_member(target);
                }
            }
            for group in concept.member_groups.iter() {
                if let Some(name) = declared_name(&group.name) {
                    push_member(name);
                }
                for member in group.members.iter() {
                    if let Some(target) = declared_name(&member.target) {
                        push_member(target);
                    }
                }
            }
            out
        }
```

`UmlSyntaxKind::LayoutWordToken` already has an arm in `fixed_vocabulary`;
the two providers both contribute and `completions` merges, sorts and dedups
them.

For `- transitions to |`, read the token kind the locator actually reports —
add a case to `crates/waml/tests/uml_completion_locator.rs` first if the table
does not already record it. If that token is `TargetToken` (the `FlowTarget`
slot reuses the shared target token), do **not** add a second `TargetToken`
arm; instead branch inside the Task 8 arm on the slot:

```rust
        UmlSyntaxKind::SourceToken
        | UmlSyntaxKind::TargetToken
        | UmlSyntaxKind::ReturnTargetToken => {
            if expectation.slot == UmlSyntaxKind::FlowTarget {
                return concept
                    .flow_nodes
                    .iter()
                    .filter_map(|node| declared_name(&node.identity))
                    .map(|identity| reference(identity, replace, "flow node"))
                    .collect();
            }
            let mut out = Vec::new();
            for lifeline in concept.lifelines.iter() {
                if let Some(alias) = declared_name(&lifeline.alias) {
                    out.push(reference(alias, replace, "lifeline handle"));
                }
            }
            for gate in concept.gates.iter() {
                if let Some(name) = declared_name(&gate.name) {
                    out.push(reference(&format!("@{name}"), replace, "local gate"));
                }
            }
            for use_ in concept.interaction_uses.iter() {
                if let Some(alias) = declared_name(&use_.alias) {
                    out.push(reference(
                        &format!("{alias}@"),
                        replace,
                        "gate on an interaction use",
                    ));
                }
            }
            out.push(reference("outside", replace, "outside the frame"));
            out
        }
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p waml --test uml_completions`
Expected: PASS.

If `transitions_to_offers_declared_flow_node_identities` or
`layout_offers_diagram_member_names` returns an empty list, check the locator
table for a `KNOWN GAP` on that construct and, if one is recorded, delete the
test and note it in the commit body.

- [ ] **Step 5: Run the gate**

Run: `cargo test --workspace && cargo clippy --workspace --all-targets --all-features -- -D warnings && cargo fmt --all -- --check`
Expected: green.

- [ ] **Step 6: Commit**

```bash
git add crates/waml/src/uml/complete.rs crates/waml/tests/uml_completions.rs
git commit -m "feat(uml): offer declared ids, flow targets and layout members

The call-id declaration site (as) and reference site (for) are different
slot kinds in the tree, so the provider tells them apart on slot kind
alone with no keyword test beside it."
```

---

### Task 10: The accept-set invariant

**Files:**
- Create: `crates/waml/tests/uml_completion_accept_set.rs`

**Interfaces:**
- Consumes: `completions` (Task 5), `in_document_refs` behaviour (Tasks 8–9).
- Produces: nothing. Tests only.

The invariant the design pins:

> Every candidate a provider offers at a position must be a value the corresponding diagnostic accepts at that position.

This is what keeps provider and diagnostic from drifting. It is a property test
over a small fixture corpus, not the whole repo: each candidate is inserted and
the document re-analysed, so the cost is candidates × documents.

- [ ] **Step 1: Write the test**

Create `crates/waml/tests/uml_completion_accept_set.rs`:

```rust
//! Accept-set invariant: a candidate a provider offers at a position must be a
//! value the diagnostic guarding that position accepts. Each candidate is
//! applied to the document and the result re-analysed; the guarding diagnostic
//! codes must not appear.

use waml::analysis::{prepare_candidate, PreparedCandidate, TextSize};
use waml::diagnostic::DiagCode;
use waml::source::{BundlePath, SourceBundle};
use waml::uml::{completions, ActionContext};

/// The codes that reject a wrong value at one of the positions the providers
/// answer. A candidate that makes any of these fire is a drift bug.
const GUARDS: &[DiagCode] = &[
    DiagCode::UnknownLifelineHandle,
    DiagCode::UnknownSequenceEndpoint,
    DiagCode::InvalidSequenceEndpoint,
    DiagCode::UnknownFlowTarget,
    DiagCode::UnknownCallIdentity,
    DiagCode::UnresolvedLayoutRef,
    DiagCode::UnresolvedTarget,
    DiagCode::SlotUnknownAttribute,
];

const SUPPORT: [(&str, &str); 2] = [
    ("a.md", "---\ntype: uml.Class\ntitle: A\n---\n# A\n"),
    ("b.md", "---\ntype: uml.Class\ntitle: B\n---\n# B\n"),
];

fn candidate_for(text: &str, revision: u64) -> PreparedCandidate {
    let mut pairs = vec![("doc.md", text)];
    pairs.extend(SUPPORT);
    prepare_candidate(SourceBundle::try_from_pairs(pairs).unwrap(), None, revision).unwrap()
}

fn guard_codes(text: &str) -> Vec<DiagCode> {
    candidate_for(text, 2)
        .uml()
        .diagnostics
        .iter()
        .map(|diagnostic| diagnostic.code)
        .filter(|code| GUARDS.contains(code))
        .collect()
}

/// Every offset in `text` at which a candidate is offered, paired with its
/// candidates. Sweeping every offset is what makes this a property test rather
/// than a hand-picked table.
fn offered_everywhere(text: &str) -> Vec<(usize, Vec<(String, std::ops::Range<usize>)>)> {
    let candidate = candidate_for(text, 1);
    let id = candidate
        .okf()
        .catalog
        .id_for_path(&BundlePath::parse("doc.md").unwrap())
        .unwrap();
    (0..=text.len())
        .filter(|offset| text.is_char_boundary(*offset))
        .filter_map(|offset| {
            let found = completions(
                ActionContext::from_prepared(&candidate).unwrap(),
                id,
                TextSize::try_from_usize(offset).unwrap(),
            )
            .unwrap();
            (!found.is_empty()).then(|| {
                (
                    offset,
                    found
                        .into_iter()
                        .map(|completion| {
                            (
                                completion.insert.to_string(),
                                completion.replace.start().to_usize()
                                    ..completion.replace.end().to_usize(),
                            )
                        })
                        .collect(),
                )
            })
        })
        .collect()
}

fn corpus() -> Vec<String> {
    vec![
        // A sequence with a half-typed endpoint.
        concat!(
            "---\ntype: uml.Sequence\ntitle: S\n---\n# S\n\n",
            "## Lifelines\n\n- [A](./a.md) as buyer\n- [B](./b.md) as order\n\n",
            "## Messages\n\n- buyer calls order `submit()` as submission\n- buyer calls \n"
        )
        .to_owned(),
        // An activity with an unfinished transition.
        concat!(
            "---\ntype: uml.Activity\ntitle: F\n---\n# F\n\n",
            "## Nodes\n\n### Receive\n\n- transitions to Check\n\n### Check\n\n- transitions to \n"
        )
        .to_owned(),
        // A diagram with an unfinished layout statement.
        concat!(
            "---\ntype: Diagram\ntitle: D\nprofile: uml-domain\n---\n# D\n\n",
            "## Members\n\n- [A](./a.md)\n- [B](./b.md)\n\n## Layout\n\n- A above \n"
        )
        .to_owned(),
        // A class with an unfinished relationship.
        concat!(
            "---\ntype: uml.Class\ntitle: C\n---\n# C\n\n",
            "## Relationships\n\n-  [A](./a.md)\n"
        )
        .to_owned(),
    ]
}

#[test]
fn every_offered_candidate_is_accepted_by_the_diagnostic_that_guards_its_position() {
    for text in corpus() {
        let baseline = guard_codes(&text);
        for (offset, candidates) in offered_everywhere(&text) {
            for (insert, replace) in candidates {
                let mut applied = text.clone();
                applied.replace_range(replace.clone(), &insert);
                let after = guard_codes(&applied);
                let introduced = after
                    .iter()
                    .filter(|code| !baseline.contains(code))
                    .collect::<Vec<_>>();
                assert!(
                    introduced.is_empty(),
                    "offset {offset}: inserting {insert:?} over {replace:?} raised {introduced:?}\n--- document ---\n{applied}"
                );
            }
        }
    }
}
```

- [ ] **Step 2: Run it**

Run: `cargo test -p waml --test uml_completion_accept_set`
Expected: PASS.

**A failure here is a real finding, not a flaky test.** It means a provider
offers something the corresponding diagnostic rejects. Fix the *provider*
(narrow the candidate set); never widen `GUARDS` or delete a corpus entry to
make it pass. If a candidate is legitimate but the diagnostic is wrong, stop
and report it — that is a diagnostic bug, out of scope for this plan.

- [ ] **Step 3: Run the gate**

Run: `cargo test --workspace && cargo clippy --workspace --all-targets --all-features -- -D warnings && cargo fmt --all -- --check`
Expected: green. This test sweeps every offset of four documents and re-analyses
per candidate; if it dominates the suite runtime, trim the corpus to the two
richest documents rather than sampling offsets — the offset sweep is the part
that finds drift.

- [ ] **Step 4: Commit**

```bash
git add crates/waml/tests/uml_completion_accept_set.rs
git commit -m "test(uml): pin the completion accept-set invariant

Every candidate offered at a position is inserted and the document
re-analysed; the diagnostic guarding that position must not fire. This is
what keeps provider and diagnostic from drifting, and it is the part of
Roslyn's problem this design does not have to solve by hand."
```

---

### Task 11: Link targets inside `](…)`

**Files:**
- Modify: `crates/waml/src/uml/complete.rs`
- Test: `crates/waml/tests/uml_completions.rs` (append)

**Interfaces:**
- Consumes: `context.okf().catalog`, `crate::okf::resolve_href`, `crate::model::ElementType`.
- Produces: private `fn link_targets(expectation: &Expectation, context: &ActionContext<'_>, from: &BundlePath) -> Vec<Completion>`, wired into `completions`.

- [ ] **Step 1: Write the failing test**

Append to `crates/waml/tests/uml_completions.rs`:

```rust
#[test]
fn a_link_target_offers_catalog_documents_labelled_by_title() {
    let offered = labels(&sequence("## Lifelines\n\n- [Buyer](|)\n"));
    let links = offered
        .iter()
        .filter(|(_, kind)| *kind == CompletionKind::Link)
        .map(|(label, _)| label.as_str())
        .collect::<Vec<_>>();
    assert!(links.contains(&"A"), "{links:?}");
    assert!(links.contains(&"B"), "{links:?}");
    assert!(!links.contains(&"S"), "a document must not link to itself: {links:?}");
}

#[test]
fn a_link_target_inserts_a_bundle_relative_path_that_round_trips() {
    let marked = sequence("## Lifelines\n\n- [Buyer](|)\n");
    let offset = marked.find('|').unwrap();
    let text = marked.replacen('|', "", 1);
    let candidate = prepared(&text, 3);
    let offered = completions(
        ActionContext::from_prepared(&candidate).unwrap(),
        document(&candidate),
        TextSize::try_from_usize(offset).unwrap(),
    )
    .unwrap();
    let chosen = offered
        .iter()
        .find(|completion| completion.label.as_ref() == "A")
        .expect("A is offered");
    let mut applied = text.clone();
    applied.replace_range(
        chosen.replace.start().to_usize()..chosen.replace.end().to_usize(),
        &chosen.insert,
    );
    let reanalyzed = prepared(&applied, 4);
    assert!(
        reanalyzed
            .uml()
            .diagnostics
            .iter()
            .all(|diagnostic| diagnostic.code != waml::diagnostic::DiagCode::UnresolvedTarget),
        "{:?} in {applied}",
        reanalyzed.uml().diagnostics
    );
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p waml --test uml_completions`
Expected: FAIL.

- [ ] **Step 3: Write the provider**

Append to `crates/waml/src/uml/complete.rs`:

```rust
/// Documents from the OKF catalog, filtered by what the enclosing section
/// accepts. Insert the bundle-relative path, label with the document title,
/// reusing the resolution `UnresolvedTarget` already performs so a chosen path
/// round-trips.
fn link_targets(
    expectation: &Expectation,
    context: &ActionContext<'_>,
    from: &crate::source::BundlePath,
) -> Vec<Completion> {
    if expectation.token != UmlSyntaxKind::LinkTargetToken {
        return Vec::new();
    }
    let wants_classifier = matches!(
        section_of(&expectation.node),
        Some(UmlSyntaxKind::LifelinesSection) | Some(UmlSyntaxKind::MembersSection)
    );
    let mut out = Vec::new();
    for document in context.okf().catalog.documents().values() {
        let path = document.path();
        if path == from {
            continue;
        }
        let Some(concept) = context.okf().bundle.concept(&crate::okf::id_of(path.as_str())) else {
            continue;
        };
        let element_type = crate::model::ElementType::parse(&concept.ty);
        if wants_classifier && !element_type.is_classifier() {
            continue;
        }
        let href = relative_href(from, path);
        // Round-trip guard: only offer a path that resolves back to this
        // document, so a candidate can never produce UnresolvedTarget.
        if crate::okf::resolve_href(from.as_str(), &href) != crate::okf::id_of(path.as_str()) {
            continue;
        }
        let label = concept
            .title
            .as_deref()
            .filter(|title| !title.is_empty())
            .unwrap_or_else(|| path.as_str());
        out.push(Completion {
            label: Arc::from(label),
            insert: Arc::from(href.as_str()),
            kind: CompletionKind::Link,
            detail: Some(Arc::from(path.as_str())),
            replace: expectation.prefix,
        });
    }
    out
}

/// The WAML section a node sits in, or `None` for a node outside one.
fn section_of(node: &SyntaxNode<UmlLanguage>) -> Option<UmlSyntaxKind> {
    let mut current = Some(node.clone());
    while let Some(candidate) = current {
        if matches!(
            candidate.kind(),
            UmlSyntaxKind::AttributesSection
                | UmlSyntaxKind::ValuesSection
                | UmlSyntaxKind::SlotsSection
                | UmlSyntaxKind::RelationshipsSection
                | UmlSyntaxKind::MembersSection
                | UmlSyntaxKind::LayoutSection
                | UmlSyntaxKind::FlowSection
                | UmlSyntaxKind::LifelinesSection
                | UmlSyntaxKind::MessagesSection
                | UmlSyntaxKind::GatesSection
        ) {
            return Some(candidate.kind());
        }
        current = candidate.parent();
    }
    None
}

/// A bundle-relative href from `from` to `to`, in the `./name.md` form the
/// corpus already uses.
fn relative_href(from: &crate::source::BundlePath, to: &crate::source::BundlePath) -> String {
    let depth = from.as_str().matches('/').count();
    let mut href = String::new();
    if depth == 0 {
        href.push_str("./");
    } else {
        for _ in 0..depth {
            href.push_str("../");
        }
    }
    href.push_str(to.as_str());
    href
}
```

Wire it into `completions`, after the `in_document_refs` block:

```rust
    candidates.extend(link_targets(&expectation, &context, version.path()));
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p waml --test uml_completions`
Expected: PASS. `ElementType::is_classifier` and `okf::resolve_href` already
exist; if `concept.title` is not a field on `okf::Concept`, use whatever the
struct exposes for the display title and adjust the label expression — do not
re-derive a title from the path when the concept carries one.

- [ ] **Step 5: Run the gate**

Run: `cargo test --workspace && cargo clippy --workspace --all-targets --all-features -- -D warnings && cargo fmt --all -- --check`
Expected: green, including `crates/waml/tests/uml_completion_accept_set.rs` —
the round-trip guard exists so link candidates cannot break it.

- [ ] **Step 6: Commit**

```bash
git add crates/waml/src/uml/complete.rs crates/waml/tests/uml_completions.rs
git commit -m "feat(uml): offer catalog documents inside a link target

Filtered by what the enclosing section accepts: a lifeline wants a
classifier. The inserted path is checked against okf::resolve_href before
it is offered, so a chosen candidate cannot produce UnresolvedTarget."
```

---

### Task 12: Type-driven slot names and enum values

**Files:**
- Modify: `crates/waml/src/uml/complete.rs`
- Test: `crates/waml/tests/uml_completions.rs` (append)

**Interfaces:**
- Consumes: `context.uml().projection` (`crate::model::Model`) for the target classifier's attributes and enum values.
- Produces: private `fn type_driven(expectation: &Expectation, context: &ActionContext<'_>, concept: &DeclaredConcept) -> Vec<Completion>`, wired into `completions`.

- [ ] **Step 1: Write the failing test**

Append to `crates/waml/tests/uml_completions.rs`:

```rust
const TYPED: [(&str, &str); 2] = [
    (
        "order.md",
        "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n\n## Attributes\n\n- status: Status\n- total: Number\n",
    ),
    (
        "status.md",
        "---\ntype: uml.Enum\ntitle: Status\n---\n# Status\n\n## Values\n\n- Draft\n- Placed\n",
    ),
];

fn typed_labels(marked: &str) -> Vec<(String, CompletionKind)> {
    let offset = marked.find('|').expect("the case must place a cursor");
    let text = marked.replacen('|', "", 1);
    let mut pairs = vec![("doc.md", text.as_str())];
    pairs.extend(TYPED);
    let candidate =
        prepare_candidate(SourceBundle::try_from_pairs(pairs).unwrap(), None, 6).unwrap();
    let id = candidate
        .okf()
        .catalog
        .id_for_path(&BundlePath::parse("doc.md").unwrap())
        .unwrap();
    completions(
        ActionContext::from_prepared(&candidate).unwrap(),
        id,
        TextSize::try_from_usize(offset).unwrap(),
    )
    .unwrap()
    .into_iter()
    .map(|completion| (completion.label.to_string(), completion.kind))
    .collect()
}

#[test]
fn an_inline_slot_name_offers_the_classifiers_attributes() {
    let offered = typed_labels(concat!(
        "---\ntype: uml.Object\ntitle: O\n---\n# O\n\n",
        "## Slots\n\n- [Order](./order.md) as o\n  - |\n"
    ));
    let fields = offered
        .iter()
        .filter(|(_, kind)| *kind == CompletionKind::Field)
        .map(|(label, _)| label.as_str())
        .collect::<Vec<_>>();
    assert!(fields.contains(&"status"), "{fields:?}");
    assert!(fields.contains(&"total"), "{fields:?}");
}

#[test]
fn a_slot_value_offers_the_attributes_enum_members() {
    let offered = typed_labels(concat!(
        "---\ntype: uml.Object\ntitle: O\n---\n# O\n\n",
        "## Slots\n\n- [Order](./order.md) as o\n  - status: |\n"
    ));
    let values = offered
        .iter()
        .filter(|(_, kind)| *kind == CompletionKind::Value)
        .map(|(label, _)| label.as_str())
        .collect::<Vec<_>>();
    assert!(values.contains(&"Draft"), "{values:?}");
    assert!(values.contains(&"Placed"), "{values:?}");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p waml --test uml_completions`
Expected: FAIL.

- [ ] **Step 3: Make the value half reachable**

A slot's value token is not yet a completion position. Add it to
`COMPLETION_TOKENS` in `crates/waml/src/uml/complete.rs`:

```rust
    UmlSyntaxKind::ValueToken,
```

Then add the matching case to `crates/waml/tests/uml_completion_locator.rs`, so
the locator table stays the record of what is reachable:

```rust
#[test]
fn a_slot_value_is_a_completion_position() {
    let located = locate(concat!(
        "---\ntype: uml.Object\ntitle: O\n---\n# O\n\n",
        "## Slots\n\n- [A](./a.md) as a\n  - status: |\n"
    ));
    assert_eq!(
        located.map(|(_, token, _)| token),
        Some(UmlSyntaxKind::ValueToken)
    );
}
```

Run: `cargo test -p waml --test uml_completion_locator`
Expected: PASS. If it returns `None`, record a `KNOWN GAP` and drop the value
half of this task (the name half still lands).

- [ ] **Step 4: Write the provider**

Append to `crates/waml/src/uml/complete.rs`:

```rust
/// The classifier whose attributes a slot position is describing: the
/// enclosing inline instance's target. `None` when the cursor is not inside
/// one, which is the ordinary case in a class document.
fn enclosing_instance_classifier(
    expectation: &Expectation,
    concept: &DeclaredConcept,
) -> Option<String> {
    let mut current = Some(expectation.node.clone());
    while let Some(node) = current {
        if node.kind() == UmlSyntaxKind::InlineInstance {
            let instance = concept
                .inline_instances
                .iter()
                .find(|instance| instance.syntax.0.range() == node.range())?;
            return declared_name(&instance.classifier).map(str::to_owned);
        }
        current = node.parent();
    }
    None
}

/// The name already authored in this slot, when the cursor is on the value
/// half rather than the name half.
fn slot_name_of(node: &SyntaxNode<UmlLanguage>) -> Option<String> {
    let name = crate::uml::SlotSyntax(node.clone()).name_token()?;
    (!name.flags().is_missing()).then(|| name.text().write_to_string())
}

/// Slot names from the target classifier's attributes -- the accept set
/// `SlotUnknownAttribute` already computes -- and values after `set to` from
/// the attribute's type when it resolves to an Enum.
fn type_driven(
    expectation: &Expectation,
    context: &ActionContext<'_>,
    concept: &DeclaredConcept,
) -> Vec<Completion> {
    let replace = expectation.prefix;
    let Some(classifier) = enclosing_instance_classifier(expectation, concept) else {
        return Vec::new();
    };
    let Some(target) = context.uml().projection.node(&classifier) else {
        return Vec::new();
    };
    match expectation.token {
        // The name half: every attribute the classifier declares.
        UmlSyntaxKind::IdentifierToken => target
            .attributes
            .iter()
            .map(|attribute| Completion {
                label: Arc::from(attribute.name.as_str()),
                insert: Arc::from(attribute.name.as_str()),
                kind: CompletionKind::Field,
                detail: Some(Arc::from(attribute.ty.name.as_str())),
                replace,
            })
            .collect(),
        // The value half: the members of the attribute's type, when that type
        // resolves to an Enum in the projection.
        UmlSyntaxKind::ValueToken => {
            let Some(slot_name) = slot_name_of(&expectation.node) else {
                return Vec::new();
            };
            let Some(attribute) = target
                .attributes
                .iter()
                .find(|attribute| attribute.name == slot_name)
            else {
                return Vec::new();
            };
            let type_id = attribute
                .ty
                .ref_
                .clone()
                .unwrap_or_else(|| attribute.ty.name.to_lowercase());
            let Some(type_node) = context.uml().projection.node(&type_id) else {
                return Vec::new();
            };
            type_node
                .values
                .iter()
                .map(|value| Completion {
                    label: Arc::from(value.as_str()),
                    insert: Arc::from(value.as_str()),
                    kind: CompletionKind::Value,
                    detail: Some(Arc::from(attribute.ty.name.as_str())),
                    replace,
                })
                .collect()
        }
        _ => Vec::new(),
    }
}
```

`crate::model::Node` carries the enum members as `values: Vec<String>`
(`crates/waml/src/model.rs:1006`) and the attributes as
`attributes: Vec<Attribute>` with `ty: TypeRef { name, ref_ }`
(`crates/waml/src/model.rs:88-102`), so both halves read the projection
directly. Do not reparse the enum document to get its members.

Wire it into `completions`, inside the existing `if let Some(concept)` block:

```rust
        candidates.extend(type_driven(&expectation, &context, concept));
```

- [ ] **Step 5: Run test to verify it passes**

Run: `cargo test -p waml --test uml_completions`
Expected: PASS.

- [ ] **Step 6: Run the gate**

Run: `cargo test --workspace && cargo clippy --workspace --all-targets --all-features -- -D warnings && cargo fmt --all -- --check`
Expected: green, including the accept-set invariant — `SlotUnknownAttribute` is
in its `GUARDS` list, so this provider is checked against the diagnostic that
owns the same accept set.

- [ ] **Step 7: Commit**

```bash
git add crates/waml/src/uml/complete.rs crates/waml/tests/uml_completion_locator.rs crates/waml/tests/uml_completions.rs
git commit -m "feat(uml): offer slot names and enum values from the target type

The slot-name candidates are the accept set SlotUnknownAttribute already
computes; the accept-set invariant test holds the two together."
```

---

### Task 13: Derived names at declaration sites

**Files:**
- Modify: `crates/waml/src/uml/complete.rs`
- Test: `crates/waml/tests/uml_completions.rs` (append)

**Interfaces:**
- Consumes: `Expectation`, `DeclaredConcept`.
- Produces: private `fn derived_names(expectation: &Expectation, concept: &DeclaredConcept) -> Vec<Completion>`, wired into `completions`.

The positions where the author invents a name rather than referencing one:
`LifelineAlias`, `InteractionUseAlias`, the inline-instance name, and
`MessageCallId`. From the link title, offer the kebab form, the lower-cased
first word, and the initials, minus any name already taken in the document —
duplicates are themselves diagnosed (`DuplicateSequenceName`,
`DuplicateCallIdentity`). With no link to derive from, offer nothing rather
than guess.

- [ ] **Step 1: Write the failing test**

Append to `crates/waml/tests/uml_completions.rs`:

```rust
#[test]
fn a_lifeline_alias_is_derived_from_the_link_title() {
    let bundle = concat!(
        "---\ntype: uml.Sequence\ntitle: S\n---\n# S\n\n",
        "## Lifelines\n\n- [Source Bundle](./a.md) as |\n"
    );
    let offered = labels(bundle);
    let names = offered
        .iter()
        .filter(|(_, kind)| *kind == CompletionKind::Name)
        .map(|(label, _)| label.as_str())
        .collect::<Vec<_>>();
    assert!(names.contains(&"source-bundle"), "{names:?}");
    assert!(names.contains(&"source"), "{names:?}");
    assert!(names.contains(&"sb"), "{names:?}");
}

#[test]
fn a_name_already_taken_in_the_document_is_not_offered_again() {
    let offered = labels(concat!(
        "---\ntype: uml.Sequence\ntitle: S\n---\n# S\n\n",
        "## Lifelines\n\n- [Source Bundle](./b.md) as source\n",
        "- [Source Bundle](./a.md) as |\n"
    ));
    let names = offered
        .iter()
        .filter(|(_, kind)| *kind == CompletionKind::Name)
        .map(|(label, _)| label.as_str())
        .collect::<Vec<_>>();
    assert!(!names.contains(&"source"), "already taken: {names:?}");
    assert!(names.contains(&"source-bundle"), "{names:?}");
}

#[test]
fn with_no_link_to_derive_from_nothing_is_offered_rather_than_a_guess() {
    let offered = labels(concat!(
        "---\ntype: uml.Sequence\ntitle: S\n---\n# S\n\n",
        "## Lifelines\n\n- [](./a.md) as |\n"
    ));
    assert!(
        !offered
            .iter()
            .any(|(_, kind)| *kind == CompletionKind::Name),
        "{offered:?}"
    );
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p waml --test uml_completions`
Expected: FAIL.

- [ ] **Step 3: Write the provider**

Append to `crates/waml/src/uml/complete.rs`:

```rust
/// The three name shapes a link title suggests: the kebab form, the lower-cased
/// first word, and the initials. Order is the order they are offered in.
fn name_shapes(title: &str) -> Vec<String> {
    let words = title
        .split_whitespace()
        .map(|word| {
            word.chars()
                .filter(|character| character.is_alphanumeric())
                .collect::<String>()
                .to_lowercase()
        })
        .filter(|word| !word.is_empty())
        .collect::<Vec<_>>();
    if words.is_empty() {
        return Vec::new();
    }
    let mut shapes = vec![words.join("-")];
    shapes.push(words[0].clone());
    if words.len() > 1 {
        shapes.push(
            words
                .iter()
                .filter_map(|word| word.chars().next())
                .collect::<String>(),
        );
    }
    shapes.dedup();
    shapes
}

/// Positions where the author invents a name rather than referencing one.
fn derived_names(expectation: &Expectation, concept: &DeclaredConcept) -> Vec<Completion> {
    let detail = match expectation.slot {
        UmlSyntaxKind::LifelineAlias => "lifeline handle",
        UmlSyntaxKind::InteractionUseAlias => "interaction use alias",
        UmlSyntaxKind::MessageCallId => "call id",
        UmlSyntaxKind::InlineInstance => "instance name",
        _ => return Vec::new(),
    };
    let Some(title) = sibling_link_title(&expectation.node) else {
        // Nothing to derive from: offer nothing rather than guess.
        return Vec::new();
    };
    let taken = concept
        .lifelines
        .iter()
        .filter_map(|lifeline| declared_name(&lifeline.alias))
        .chain(
            concept
                .interaction_uses
                .iter()
                .filter_map(|use_| declared_name(&use_.alias)),
        )
        .chain(
            concept
                .messages
                .iter()
                .filter_map(|message| declared_name(&message.call_id)),
        )
        .chain(
            concept
                .inline_instances
                .iter()
                .filter_map(|instance| declared_name(&instance.name)),
        )
        .map(str::to_owned)
        .collect::<Vec<_>>();
    name_shapes(&title)
        .into_iter()
        .filter(|shape| !taken.contains(shape))
        .map(|shape| Completion {
            label: Arc::from(shape.as_str()),
            insert: Arc::from(shape.as_str()),
            kind: CompletionKind::Name,
            detail: Some(Arc::from(detail)),
            replace: expectation.prefix,
        })
        .collect()
}
```

and the helper it needs, beside `section_of`:

```rust
/// The title text of the link on the same bullet as `node`: walk up to the
/// bullet, find its `Link` child, and read the `LinkTextToken`. `None` when
/// the token is missing or empty -- with nothing to derive from, the caller
/// offers nothing rather than a guess.
fn sibling_link_title(node: &SyntaxNode<UmlLanguage>) -> Option<String> {
    let mut current = node.parent();
    while let Some(bullet) = current {
        if matches!(
            bullet.kind(),
            UmlSyntaxKind::Lifeline
                | UmlSyntaxKind::InteractionUse
                | UmlSyntaxKind::Message
                | UmlSyntaxKind::InlineInstance
        ) {
            let link = bullet
                .children()
                .filter_map(SyntaxElement::into_node)
                .find(|child| child.kind() == UmlSyntaxKind::Link)?;
            let text = link
                .children()
                .filter_map(SyntaxElement::into_token)
                .find(|token| token.kind() == UmlSyntaxKind::LinkTextToken)?;
            if text.flags().is_missing() {
                return None;
            }
            let title = text.text().write_to_string();
            return (!title.trim().is_empty()).then_some(title);
        }
        current = bullet.parent();
    }
    None
}
```

The lifeline case is the one the spec names:
`- [Source Bundle](../concepts/implementation/source-bundle.md) as ` offers
`source-bundle`, `source` and `sb`.

Wire it into `completions`, inside the `if let Some(concept)` block:

```rust
        candidates.extend(derived_names(&expectation, concept));
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p waml --test uml_completions`
Expected: PASS.

- [ ] **Step 5: Run the gate**

Run: `cargo test --workspace && cargo clippy --workspace --all-targets --all-features -- -D warnings && cargo fmt --all -- --check`
Expected: green.

- [ ] **Step 6: Commit**

```bash
git add crates/waml/src/uml/complete.rs crates/waml/tests/uml_completions.rs
git commit -m "feat(uml): derive candidate names from the link title

At the positions where an author invents a name -- lifeline and use
aliases, inline instance names, call ids -- offer the kebab form, the
first word and the initials, minus any name already taken. With no link
to derive from, offer nothing rather than guess."
```

---

## Deferred work

#### Slice 5: the native editor popup — deferred, not planned as a task

The spec stages a native-editor popup last, "with the visual check deferred".
This plan defers the whole slice, for two reasons that a task boundary cannot
work around:

1. **It cannot be verified headlessly.** The implementer runs without a display.
   Any task whose acceptance is "the popup looks right" stalls indefinitely.
2. **The halfway version is dead code.** `crates/waml-editor/src/lib.rs` keeps
   `mod documents;` crate-private on purpose (see its dead-code note): adding a
   `completion_candidates` function there without the widget that calls it fails
   `-D warnings`, and adding the widget is the visual work.

The follow-up, when it is picked up, is: `crates/waml-editor/src/documents.rs`
gains a `pub(crate) fn completions_at(open: &OpenDocument, offset: TextSize)`
that calls `waml::uml::completions` exactly as its test at `documents.rs:522`
already calls `repair_actions`; a popup widget consumes it; and **a visual
check is owed to the user** — the popup's placement, keyboard handling and
dismissal cannot be signed off from a headless run.

#### Frontmatter `type:` values — deferred, with a reason

The spec lists frontmatter `type:` values under `fixed_vocabulary` and puts
"completion inside frontmatter beyond the `type:` value" out of scope. The
locator cannot reach that position: the UML parser wraps the whole frontmatter
block in a `MarkdownRegion` of `RawMarkdownToken`s
(`crates/waml/src/uml/syntax/parser.rs:17-54`), so there is no typed slot for
the `type:` value, and `crates/waml/src/frontmatter.rs` exposes no ranges. The
element-type words therefore have no completion position under this design.
Offering them needs a frontmatter range facility that does not exist; that is a
separate, small piece of work and it is not in this plan. Nothing else in the
spec depends on it.

---

## Notes for the implementer

- **Run the full gate before every commit**, not just the task's own test. The
  six commands are listed under Global Constraints.
- **`crates/waml-syntax/tests/properties.proptest-regressions` must never be
  committed.** If the `waml-syntax` proptest goes red and appends to that file,
  that failure is pre-existing and unrelated — `git checkout`/delete the file
  and move on. Do not investigate it as part of this work.
- **Never add a keyword list.** If a task seems to need one, it belongs in
  `crates/waml/src/uml/vocabulary.rs` and everything reads it from there.
- **Never make completion read a diagnostic, or a diagnostic read completion.**
  They share slot-kind tables, not each other.
- **Locator behaviour is the contract.** If a construct turns out not to have a
  fixed missing-token slot, record it as a `KNOWN GAP` in
  `crates/waml/tests/uml_completion_locator.rs` and carry on. Changing the
  parser's slot shape is a separate decision that needs the user.
