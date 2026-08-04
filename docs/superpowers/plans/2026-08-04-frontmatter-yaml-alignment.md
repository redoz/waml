# Frontmatter YAML Alignment Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make waml's frontmatter parse the YAML people actually write (Tier B block structure, YAML 1.2 core scalars), render it in the editor as code with syntax coloring, and keep the writer's round-trip invariant intact.

**Architecture:** The green/red tree gains nested `FrontmatterMapping` / `FrontmatterSequence` nodes built by an indent-stack builder inside the existing classifier/builder split in `waml-syntax/src/markdown/parser.rs`. Scalar classification (YAML 1.2 core) lives in `waml-syntax` and is the single source of truth for both the model (`waml/src/frontmatter.rs`) and the editor colors (`waml-markdown-editor/src/presentation/compile.rs`). `FmValue` gains `Map` and `Null`; the writer renders nested maps as block mappings and sequences as flow (block only for sequence-of-maps). The round-trip proptest lands before any writer change.

**Tech Stack:** Rust workspace; proptest (already a dep of both `waml-syntax` and `waml`); golden fixtures under `crates/waml-syntax/tests/fixtures/shell/` driven by `crates/waml-syntax/src/markdown/parser_tests.rs` (`WAML_DUMP_SHELL_GOLDENS` env var dumps actuals for blessing — read them before blessing).

**Spec:** `docs/superpowers/specs/2026-08-04-frontmatter-yaml-alignment-design.md` — read it before starting any task; every rule below is quoted from it.

## Global Constraints

- Gate for EVERY task: `cargo test --workspace` green (run from the worktree root `C:/dev/waml/.worktrees/frontmatter-yaml-spec`). No task touches `editors/vscode`.
- Before Task 1, run the gate once at the baseline commit and record any pre-existing failures (local main has previously carried 2 unrelated icon-table failures). A pre-existing failure is not yours to fix, but nothing you add may fail.
- Commit messages: conventional-commit subject + body. NO co-author trailer, NO "Generated with" footer.
- Every commit uses absolute paths in `git -C C:/dev/waml/.worktrees/frontmatter-yaml-spec ...` or runs with that cwd. Never edit files under `C:/dev/waml/crates/...` (the MAIN checkout) — only under the worktree.
- NO YAML 1.1: `NO`/`no`/`yes`/`on`/`off`/`y`/`n` are strings. No flow mappings (`{a: 1}`). No anchors, aliases, tags, directives, multi-docs, no date type.
- Full fidelity: every byte of the source stays in the tree; `render_to_string` of the tree reproduces the source exactly. Indentation stays leading trivia.
- No `unwrap`/`expect` reachable from document content. Recovery is per line and never fatal: unreadable line → `BadToken` inside its `FrontmatterEntry`, indent stack keeps state, region still yields a tree, `FrontmatterNotClean` marks it.
- Headless boundary: nothing in `waml` or `waml-syntax` may depend on the editor. The scalar classifier goes in `waml-syntax`, not in the editor.
- `clippy -D warnings` in the gate promotes dead code to errors: never land an enum variant, function, or module with no producer/consumer in the same task.

---

### Task 1: Presentation quick fix — frontmatter gets a Code block and stays visible

Standalone fix the user wants available separately; depends on nothing else in this plan.

**Files:**
- Modify: `crates/waml-markdown-editor/src/presentation/compile.rs` (`block_kind`, ~line 615)
- Modify: `crates/waml-markdown-editor/src/presentation/mod.rs` (`TextRole::is_syntax_marker`, ~line 87)
- Test: `crates/waml-markdown-editor/src/presentation/compile.rs` tests module (or the presentation test module the existing `block_kind` tests live in — search for existing `PresentationBlockKind` assertions and put these beside them)

**Interfaces:**
- Consumes: existing `PresentationBlockKind::Code`, `OkfMarkdownSyntaxKind::Frontmatter`.
- Produces: frontmatter compiles to its own `PresentationBlockKind::Code` block (later tasks' presentation tests rely on this); `TextRole::Frontmatter.is_syntax_marker() == false`.

- [ ] **Step 1: Write two failing tests**

In the presentation test module (grep for where `block_kind` / compiled blocks are asserted; follow that harness style — compile a document and inspect blocks):

```rust
#[test]
fn frontmatter_compiles_to_its_own_code_block() {
    // Use the same compile-a-document helper the neighbouring tests use.
    let source = "---\ntype: uml.Class\n---\n\nBody paragraph.\n";
    let compiled = compile_test_document(source); // existing helper in this module
    assert!(
        compiled.blocks.iter().any(|b| b.kind == PresentationBlockKind::Code
            /* and the block's range covers the frontmatter region 0..len("---\ntype: uml.Class\n---\n") */),
        "frontmatter must be its own Code block, not folded into the root paragraph"
    );
}

#[test]
fn frontmatter_role_is_not_a_syntax_marker() {
    assert!(!TextRole::Frontmatter.is_syntax_marker());
}
```

Adapt the first test's assertions to the actual compiled-block shape used by neighbouring tests — assert (a) a block exists whose kind is `Code` and whose range is the frontmatter range, and (b) no frontmatter run falls under the document-root paragraph.

- [ ] **Step 2: Run to verify both fail**

Run: `cargo test -p waml-markdown-editor frontmatter_compiles_to_its_own_code_block frontmatter_role_is_not_a_syntax_marker`
Expected: FAIL (no Code block for frontmatter; `is_syntax_marker` returns true).

- [ ] **Step 3: Implement**

In `block_kind` (`compile.rs` ~line 622), add an arm to the `match node.kind()`:

```rust
Kind::Frontmatter => PresentationBlockKind::Code,
```

(If `PresentationBlockKind::Code` carries payload — check the `FencedCodeBlock` arm in the same match and mirror whatever it constructs, minus any info-string language.)

In `mod.rs` `is_syntax_marker` (~line 98), delete `| Self::Frontmatter` from the `matches!`. Update the doc comment above it (~line 83): remove the sentence claiming `Frontmatter` is included; state that fences and body are always visible in the editor and reading presentation is the separate viewer's concern.

- [ ] **Step 4: Run the full gate**

Run: `cargo test --workspace`
Expected: PASS. If an existing presentation/layout test asserted the old root-paragraph behaviour or `hide_syntax` hiding frontmatter, update that test to the new intended behaviour (the spec says this removes the borrowed-`line_spacing_scale` path deliberately).

- [ ] **Step 5: Commit**

```bash
git add crates/waml-markdown-editor
git commit -m "fix(presentation): give frontmatter its own code block and keep it visible

Kind::Frontmatter now maps to PresentationBlockKind::Code, so its runs get
fixed code pitch instead of being rebuilt into a root paragraph with a
borrowed line_spacing_scale. Frontmatter is dropped from is_syntax_marker:
the editor always shows fences and body; hiding is the viewer's concern."
```

---

### Task 2: Round-trip property test for the CURRENT FmValue (the writer guard)

Must land before ANY change to `parse_value` / `scalar_needs_quote` / `render_*` — a writer bug corrupts documents on save; this proptest is the tripwire. It tests today's variants only (`Str`, `Bool`, `Num`, `List`); Task 8 extends it.

**Files:**
- Modify: `crates/waml/src/frontmatter.rs` (tests module only)

**Interfaces:**
- Produces: `fn fm_value_strategy() -> impl Strategy<Value = FmValue>` inside the tests module — Tasks 7/8 extend this strategy in place. Property: `parse_value(&render_value(&v)) == v`.

- [ ] **Step 1: Write the proptest**

At the bottom of the `tests` module in `crates/waml/src/frontmatter.rs`:

```rust
use proptest::prelude::*;

fn fm_value_strategy() -> impl Strategy<Value = FmValue> {
    let leaf = prop_oneof![
        any::<bool>().prop_map(FmValue::Bool),
        // Finite, non-NaN, and re-parseable: round through the renderer's own
        // formatting so 1.0 vs 1 formatting differences don't fail spuriously.
        (-1.0e9f64..1.0e9).prop_map(|n| FmValue::Num((n * 1000.0).round() / 1000.0)),
        // Printable ASCII plus some unicode; includes strings that LOOK like
        // bools, numbers, lists, quotes, backslashes — the quoting stress cases.
        prop_oneof![
            "[ -~]{0,24}",
            "(true|false|null|~|yes|NO|on|off)",
            "-?[0-9]{1,6}(\\.[0-9]{1,3})?",
            "\\[[a-z, ]{0,10}\\]?",
            "[\"'\\\\#:>|&*!%@`,\\-\\[\\]{} ]{1,8}",
            "\\PC{0,12}",
        ]
        .prop_map(FmValue::Str),
    ];
    leaf.prop_recursive(4, 48, 8, |inner| {
        prop::collection::vec(inner, 0..6).prop_map(FmValue::List)
    })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1024))]

    /// THE writer contract: parse_value(render_value(v)) == v, for every value.
    #[test]
    fn rendered_value_reparses_identically(v in fm_value_strategy()) {
        let rendered = render_value(&v);
        prop_assert_eq!(parse_value(&rendered), v.clone(), "rendered as {:?}", rendered);
    }

    /// Entry-level contract: a whole rendered frontmatter reparses line by line.
    #[test]
    fn rendered_frontmatter_entries_reparse(
        entries in prop::collection::vec(("[a-z][a-z0-9_]{0,8}", fm_value_strategy()), 0..5)
    ) {
        let fm = Frontmatter { entries: entries.clone() };
        let rendered = render_frontmatter(&fm);
        for (line, (key, value)) in rendered.lines().zip(&fm.entries) {
            let value_text = line.strip_prefix(&format!("{key}: "))
                .or_else(|| line.strip_prefix(&format!("{key}:")))
                .unwrap_or("");
            prop_assert_eq!(&parse_value(value_text), value, "line {:?}", line);
        }
    }
}
```

NOTE: never commit a `proptest-regressions` file (repo rule). If the strategy finds a real pre-existing round-trip bug in current code (possible — e.g. a `Str` whose rendered bare form re-parses differently), do NOT silently narrow the strategy: minimize the case, and either fix it in this task if it is a one-line `scalar_needs_quote` gap (e.g. adding a missing character to the quote predicate), or record it as an explicit `#[ignore]`d named regression test with a comment pointing at Task 8, and constrain the strategy with a comment naming the exclusion. Prefer the fix.

- [ ] **Step 2: Run it**

Run: `cargo test -p waml frontmatter::tests -- --nocapture`
Expected: PASS against current code (or see the note above).

- [ ] **Step 3: Run the full gate**

Run: `cargo test --workspace` — Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add crates/waml/src/frontmatter.rs
git commit -m "test(frontmatter): add round-trip proptest guarding the writer

parse_value(render_value(v)) == v is the invariant the writer can silently
break, corrupting documents on save. Pin it with a proptest over arbitrary
FmValue trees before any writer change lands. Also asserts the entry-level
render_frontmatter form reparses line by line."
```

---

### Task 3: Shared scalar classifier in waml-syntax (YAML 1.2 core)

The single source of truth both the model and the editor colors call. Pure functions, no tree dependency, so it lands green with its own tests and its first consumer arrives in Tasks 7/9 — to avoid dead-code lint it is `pub` API of the crate from day one and its table test is its consumer.

**Files:**
- Create: `crates/waml-syntax/src/markdown/scalar.rs`
- Modify: `crates/waml-syntax/src/markdown/mod.rs` (add `pub mod scalar;` following the existing module list style)
- Modify: `crates/waml-syntax/src/lib.rs` (re-export: `pub use markdown::scalar::{classify_bare_scalar, FrontmatterScalarKind};` — follow the crate's existing re-export style)

**Interfaces:**
- Produces (exact signatures later tasks consume):

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FrontmatterScalarKind { Null, Bool, Number, Str }

/// Classify a BARE (unquoted, trimmed) scalar under the YAML 1.2 core schema,
/// minus the legacy misfeatures waml rejects (no 1.1 bool words, no dates).
pub fn classify_bare_scalar(s: &str) -> FrontmatterScalarKind;
```

- [ ] **Step 1: Write the table test first (failing — module does not exist)**

In `scalar.rs` (tests module at the bottom):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use FrontmatterScalarKind::*;

    #[test]
    fn yaml_12_core_scalar_table() {
        // The spec's table, one row per rule. The Norway case is pinned by name.
        let table: &[(&str, FrontmatterScalarKind)] = &[
            ("true", Bool), ("false", Bool),
            // NOT bools — YAML 1.2 core dropped the 1.1 words (Norway problem):
            ("NO", Str), ("no", Str), ("yes", Str), ("on", Str), ("off", Str),
            ("y", Str), ("n", Str), ("True", Str), ("FALSE", Str),
            ("null", Null), ("~", Null), ("", Null),
            ("0", Number), ("42", Number), ("-7", Number), ("+7", Number),
            ("0x1A", Number), ("0o17", Number),
            ("3.5", Number), ("-3.5", Number), ("6.02e23", Number),
            (".5", Number), ("10.", Number), ("1e3", Number),
            (".inf", Number), ("-.inf", Number), ("+.inf", Number), (".nan", Number),
            // Date-shaped stays a string (no date type):
            ("2026-08-04", Str),
            ("uml.Class", Str), ("a b", Str), ("Null", Str), ("nan", Str),
            ("0x", Str), ("0o8", Str), ("1e", Str), ("-", Str), (".", Str),
        ];
        for (input, expected) in table {
            assert_eq!(classify_bare_scalar(input), *expected, "input {input:?}");
        }
    }
}
```

- [ ] **Step 2: Run to verify it fails to compile**

Run: `cargo test -p waml-syntax scalar` — Expected: compile error, module missing content.

- [ ] **Step 3: Implement**

```rust
use regex::Regex;
use std::sync::LazyLock;

static INT_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[-+]?[0-9]+$").unwrap());
static HEX_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^0x[0-9a-fA-F]+$").unwrap());
static OCT_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^0o[0-7]+$").unwrap());
static FLOAT_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[-+]?(\.[0-9]+|[0-9]+(\.[0-9]*)?)([eE][-+]?[0-9]+)?$").unwrap());

pub fn classify_bare_scalar(s: &str) -> FrontmatterScalarKind {
    use FrontmatterScalarKind::*;
    match s {
        "" | "null" | "~" => return Null,
        "true" | "false" => return Bool,
        ".inf" | "-.inf" | "+.inf" | ".nan" => return Number,
        _ => {}
    }
    if INT_RE.is_match(s) || HEX_RE.is_match(s) || OCT_RE.is_match(s) || FLOAT_RE.is_match(s) {
        // FLOAT_RE admits a bare exponent-less int too; that is fine, both are Number.
        // Guard the degenerate forms the regex union would admit: "." and "e"-only
        // are excluded by the regexes themselves (\.[0-9]+ needs a digit).
        return Number;
    }
    Str
}
```

Check: `waml-syntax/Cargo.toml` must have `regex` as a dependency — grep for it; if absent, add `regex.workspace = true` (it is already a workspace dep via `waml`). If the crate deliberately avoids `regex`, implement the four matchers as hand-rolled `fn is_int(s)/is_hex(s)/is_oct(s)/is_float(s)` character scans instead — behaviour per the table is what is pinned, not the mechanism.

- [ ] **Step 4: Run test + gate**

Run: `cargo test -p waml-syntax scalar` then `cargo test --workspace` — Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/waml-syntax
git commit -m "feat(waml-syntax): add shared YAML 1.2 core scalar classifier

classify_bare_scalar is the single verdict both the model (parse_value) and
the editor coloring will call, so a value painted as a Number can never be
read as a Str. YAML 1.1 bool words (NO/yes/on/off/y/n) classify as Str by
design; date-shaped scalars stay Str. Table test pins every rule."
```

---

### Task 4: Parser — nested mappings, sequences, comments, quoted scalars

The core builder rewrite, WITHOUT block scalars (those are Task 5 because they also change the classifier/region detection). New syntax kinds land here together with their producer.

**Files:**
- Modify: `crates/waml-syntax/src/markdown/kind.rs`
- Modify: `crates/waml-syntax/src/markdown/parser.rs` (the `frontmatter` builder, `frontmatter_entry`, helpers)
- Modify: `crates/waml-syntax/src/markdown/parser_tests.rs` (register new fixtures)
- Create fixtures in `crates/waml-syntax/tests/fixtures/shell/`: `fm_nested_map.md/.golden`, `fm_block_seq_scalars.md/.golden`, `fm_block_seq_maps.md/.golden`, `fm_comments.md/.golden`, `fm_quotes.md/.golden`, `fm_indent_errors.md/.golden`

**Interfaces:**
- Consumes: existing `GreenFactory`, `line_at`/`lines`/`trivia`/`token_with_leading` helpers in `parser.rs`.
- Produces — new `OkfMarkdownSyntaxKind` variants (exact names, used by Tasks 5–9):
  - Nodes: `FrontmatterMapping`, `FrontmatterSequence`, `FrontmatterSequenceItem`
  - Tokens: `FrontmatterDashToken`, `FrontmatterQuotedValueToken`, `FrontmatterCommentToken` (`FrontmatterBlockScalarHeaderToken` arrives in Task 5)
  - Diagnostic codes: `DuplicateFrontmatterKey`, `TabInFrontmatterIndent`, `InvalidFrontmatterIndent`, `UnterminatedQuotedScalar` (`InvalidEscapeSequence` arrives in Task 7 with the decoder)
- Tree shape contract (Tasks 7 and 9 read this): `Frontmatter` = `FrontmatterOpenFence` token, exactly one `FrontmatterMapping` node, `FrontmatterCloseFence` token (possibly missing). `FrontmatterMapping` children are `FrontmatterEntry` nodes (plus comment/blank-line entries). A `FrontmatterEntry` is `FrontmatterKey` + `ColonToken` + one of: `FrontmatterValue` token (bare scalar) | `FrontmatterQuotedValueToken` | nested `FrontmatterMapping` | nested `FrontmatterSequence` | missing `FrontmatterValue` (empty → Null), optionally followed by `FrontmatterCommentToken`, then the newline. `FrontmatterSequence` children are `FrontmatterSequenceItem` nodes, each `FrontmatterDashToken` + the same value alternatives. A comment-only or blank line becomes a `FrontmatterEntry` holding just the `FrontmatterCommentToken`/newline (keeps `# a comment` from being a `BadToken`). Indentation is leading trivia on the first token of each line; every byte stays in the tree.

- [ ] **Step 1: Write the fixture sources and a unit test for the indent stack**

Fixture `fm_nested_map.md`:

```
---
type: uml.Class
meta:
  owner: platform
  detail:
    level: 3
title: Order
---

Body.
```

Fixture `fm_block_seq_scalars.md`:

```
---
stereotype:
  - aggregateRoot
  - entity
---
```

Fixture `fm_block_seq_maps.md`:

```
---
authors:
  - name: Ana
    team: platform
  - name: Bo
---
```

Fixture `fm_comments.md` (comment-only line, trailing comment, `#` inside quotes is content):

```
---
# a leading comment
title: Order # trailing comment
tag: "not # a comment"
---
```

Fixture `fm_quotes.md` (both styles, `''` escape, `a: b: c` should NOT silently pass — with the new parser a bare value containing `: ` is a `BadToken` line, since YAML errors here):

```
---
single: 'it''s literal'
double: "line\nbreak"
unterminated: "oops
mapping_lookalike: a: b
---
```

Fixture `fm_indent_errors.md` (tab indent; a column matching no open block; recovery continues):

```
---
a:
	tabbed: bad
b:
   three: ok
  two_after_three: bad
c: still parsed
---
```

Then register all six in the `FIXTURES` array in `parser_tests.rs` via the `fixture!("...")` macro, with EMPTY `.golden` files for now.

Add a focused unit test in `parser.rs`'s test module (beside `frontmatter_classifier_drives_full_and_window_consumption`) asserting the tree shape for the nested-map fixture: parse `"---\na:\n  b: 1\n---\n"`, walk to `Frontmatter` → child `FrontmatterMapping` → entry `a` whose value child is a `FrontmatterMapping` containing entry `b` with `FrontmatterValue` text `1`. Assert `render_to_string` of the root equals the source byte-for-byte.

- [ ] **Step 2: Run to verify failures**

Run: `cargo test -p waml-syntax markdown::parser` and the fixture test `shell_fixtures_are_exact_bounded_progressing_and_golden`.
Expected: FAIL (goldens empty, tree shape flat, `# a leading comment` is a BadToken today).

- [ ] **Step 3: Implement the kinds**

In `kind.rs`, add to `OkfMarkdownSyntaxKind` (beside the existing `#[doc(hidden)]` frontmatter kinds, same hidden treatment as `FrontmatterKey`/`FrontmatterValue` since the parser-facing kinds there are hidden — put nodes with the node kinds near `FrontmatterEntry` and mark them the same visibility as `FrontmatterEntry`, which is public):

```rust
    FrontmatterMapping,
    FrontmatterSequence,
    FrontmatterSequenceItem,
    ...
    FrontmatterDashToken,
    FrontmatterQuotedValueToken,
    FrontmatterCommentToken,
```

And to `OkfSyntaxDiagnosticCode`:

```rust
    DuplicateFrontmatterKey,
    TabInFrontmatterIndent,
    InvalidFrontmatterIndent,
    UnterminatedQuotedScalar,
```

Check for exhaustive `match`es on these enums across the workspace (`cargo check --workspace` will find them — `snapshot.rs` `semantic_role`/`source_role` use catch-alls so may compile; still, grep for `OkfMarkdownSyntaxKind::` matches and give the new kinds correct arms where a match is exhaustive). For this task: in `snapshot.rs`, map the three new node kinds into `is_semantic` (add beside `FrontmatterEntry`) and `semantic_role` → `MarkdownSemanticRole::Frontmatter` (Task 9 refines the roles), and `FrontmatterQuotedValueToken` into `source_role`'s `Content` arm.

- [ ] **Step 4: Implement the builder**

Rewrite the line loop in `frontmatter` (parser.rs ~line 159) as a recursive-descent-over-lines builder with an explicit stack. Skeleton (adapt to the file's real helper signatures — `line_at`, `trivia`, `token_with_leading`, `identified_node`, `newline_token` all exist):

```rust
struct FmLine { line: Line, indent: usize, has_tab_indent: bool }

/// One frame per open container. `indent` is the column of the container's
/// first entry (or first dash for a sequence).
enum FmFrame {
    Mapping { indent: usize, children: Vec<GreenElement<OkfMarkdownLanguage>> },
    Sequence { indent: usize, children: Vec<GreenElement<OkfMarkdownLanguage>> },
}
```

Algorithm, per significant line between `open.end` and `entries_end`:

1. Compute `indent` = count of leading spaces from `line.start`; if a `\t` appears in the leading run, emit `TabInFrontmatterIndent` (span = the tab run), mark `clean = false`, and treat the line as `BadToken` content inside the current container (do NOT try to guess a column).
2. Blank line or line whose first significant char is `#`: build a `FrontmatterEntry` holding optional leading trivia + `FrontmatterCommentToken` (for `#…` up to `significant_end`) + newline; push into the CURRENT top frame. Comments never open or close containers.
3. Dedent: while `indent <` top frame's `indent`, pop the frame, wrap its children in its node kind (`FrontmatterMapping`/`FrontmatterSequence`) via `identified_node`, and push that node into the parent frame — as the value child of the parent's last open entry/item (keep a "pending value slot" per frame: the entry or sequence-item node is only finalized when its value child is known). If after popping, `indent` matches no frame's column exactly, emit `InvalidFrontmatterIndent` (span = the leading-whitespace run), `clean = false`, and consume the line as a `BadToken` entry in the now-current frame. Recovery per line; the stack keeps its state.
4. `- ` or lone `-` at `indent`: if the top frame is a `Mapping` whose last entry is awaiting a value and `indent >` that entry's key column, push a `Sequence` frame at `indent`. If the top frame is a `Sequence` at this `indent`, continue it. Build a `FrontmatterSequenceItem`: `FrontmatterDashToken` for the `-`, then parse the rest of the line after the dash as a value position (rules 5–7); `- name: Ana` opens a nested `Mapping` frame at the column of `name`, making the item's value a mapping (sequence of maps).
5. `key:` with nothing after the colon (or only a comment): emit `FrontmatterKey` + `ColonToken` (+ optional `FrontmatterCommentToken`) and leave the entry AWAITING a value: the next line decides — deeper indent opens `Mapping` or `Sequence`; same/lesser indent finalizes the entry with a missing `FrontmatterValue` token (Null).
6. `key: value`: split key at the first colon FOLLOWED BY space-or-EOL (YAML rule; `a: b: c` → the value scan below flags it). Value scan, quote-aware: `'…'` / `"…"` → `FrontmatterQuotedValueToken` (unterminated by EOL → `UnterminatedQuotedScalar`, token still emitted to `significant_end`, `clean = false`); `[…]` flow sequence → keep as one `FrontmatterValue` token (flow parsing happens at the value level in Task 7; the token is the whole balanced bracket run — quote-aware bracket matching so `["a,b"]` is one token); otherwise bare scalar → `FrontmatterValue`, BUT terminated early by ` #` (whitespace then `#`) which becomes a trailing `FrontmatterCommentToken` — trailing spaces before the comment go to the comment token's leading trivia; a bare value containing `: ` (colon-space) is malformed (`MalformedFrontmatterEntry` + `BadToken` for the whole run) because YAML errors on `a: b: c`.
7. Keys may be quoted: a line starting `'…':` or `"…":` uses the quoted token as the key (emit `FrontmatterQuotedValueToken` in key position — Task 7's reader and Task 9's colorer both treat a quoted token before the colon as a key).
8. Duplicate keys within ONE mapping: track key texts per `Mapping` frame (a `Vec<String>` — frontmatter is small); on repeat, emit `DuplicateFrontmatterKey` with the span of the LATER key token. Both entries stay in the tree (fidelity); the model keeps the last (Task 7).
9. A line with no colon, no dash, no `#`: `BadToken` entry + `MalformedFrontmatterEntry` (unchanged from today).

At `entries_end`, pop all frames to depth zero, wrap, and place the single root `FrontmatterMapping` between the fence tokens. If there were zero entry lines the mapping node is present but empty. Every push/pop preserves byte coverage — reuse the existing `structured_end`/width discipline; `parse_window`'s `consumed != window.range.len()` check will catch a hole.

Also update `plausible_unclosed_frontmatter` (parser.rs ~line 333): a line that is a comment (`#…`), a sequence item (`- …`), or indented continuation must not disqualify an unclosed candidate; keep requiring at least one `key:` line at column 0.

- [ ] **Step 5: Bless goldens and make everything green**

Run: `WAML_DUMP_SHELL_GOLDENS='' cargo test -p waml-syntax shell_fixtures 2>&1` (env var dumps actuals per the harness), READ each dumped golden and verify by hand it matches the shape contract above (nested nodes, comments as tokens, BadTokens exactly where intended), then write the six `.golden` files. Existing goldens for old fixtures (`malformed_clean`, `broken-frontmatter`, `clean_lf`, …) will change where the tree shape changed — re-bless ONLY after reading each diff and confirming it is the intended reshaping (entry list → mapping node) and not a lost byte. `git diff` the goldens in the commit body if any surprises.

`cargo test --workspace` — fix fallout: `crates/waml/src/frontmatter.rs::parse_closed_syntax` reads `FrontmatterEntry` as direct children of `Frontmatter`; make it also descend through one `FrontmatterMapping` level (top-level entries only for now — full nested reading is Task 7):

```rust
let entry_parent = node
    .children()
    .filter_map(SyntaxElement::into_node)
    .find(|n| n.kind() == OkfMarkdownSyntaxKind::FrontmatterMapping);
let entries = entry_parent.as_ref().unwrap_or(node).children();
```

and accept `FrontmatterQuotedValueToken` where it accepts `FrontmatterValue` (strip quotes via the existing `decode_quoted_string` for double; for single, strip and replace `''`→`'` inline — full decoding rules land in Task 7). Any incremental tests asserting old tree shapes: update. `waml/src/uml/format.rs` and `edit/batch.rs` compile against the same functions — check they still pass.

- [ ] **Step 6: Full gate then commit**

Run: `cargo test --workspace` — Expected: PASS.

```bash
git add crates/waml-syntax crates/waml
git commit -m "feat(waml-syntax): parse nested frontmatter block structure

The frontmatter builder becomes an indent-stack parser producing
FrontmatterMapping / FrontmatterSequence / FrontmatterSequenceItem nodes,
with comments, quoted scalars (both styles), sequence dashes, duplicate-key
and indent diagnostics. Indentation stays leading trivia; every byte stays
in the tree and render_to_string reproduces the source. Recovery is per
line: a bad line is a BadToken entry and later lines still parse.
Block scalars follow separately (they also change region detection)."
```

---

### Task 5: Block scalars and the block-scalar-aware classifier

Region detection changes here — the riskiest part of the spec ("a mistake here misclassifies document bodies"). Includes the `same_frontmatter_fences` fix, because once the classifier skips a `---` inside a literal block, the incremental fence comparison must use the same scan or an edit inside a literal block miscompares.

**Files:**
- Modify: `crates/waml-syntax/src/markdown/kind.rs` (add `FrontmatterBlockScalarHeaderToken`)
- Modify: `crates/waml-syntax/src/markdown/parser.rs` (`classify_frontmatter`, the builder)
- Modify: `crates/waml-syntax/src/incremental.rs` (`frontmatter_fences`, ~line 1290)
- Modify: `crates/waml-syntax/src/markdown/snapshot.rs` (`source_role`: header token is a marker, block-scalar content lines are Content — they arrive as `FrontmatterValue` tokens, already Content)
- Create fixtures: `fm_block_scalars.md/.golden` (both styles × three chomping modes), `fm_fence_inside_block_scalar.md/.golden`

**Interfaces:**
- Consumes: Task 4's frame stack and token kinds.
- Produces: `FrontmatterBlockScalarHeaderToken` (text `|`, `>`, `|-`, `|+`, `>-`, `>+`, optionally with an explicit indent digit e.g. `|2`); block scalar content lines inside the entry as `FrontmatterValue` tokens (one per line, indentation as leading trivia). Produces the classifier fn used by BOTH `classify_frontmatter` and `incremental.rs`: `pub(crate) fn frontmatter_close_fence_line(source: &str, open_end: usize, boundary: usize) -> Option<Line>` in `parser.rs` — the block-scalar-aware close-fence search, called from both sides so they cannot drift.

- [ ] **Step 1: Write the fixtures and failing tests**

`fm_fence_inside_block_scalar.md` — the spec's own example, the guard on region detection:

```
---
description: |
  ---
key: value
---

Body.
```

Expected (assert in a unit test beside the classifier tests BEFORE blessing goldens): `frontmatter_range` covers through the real close fence on line 5; `key: value` is a frontmatter entry, NOT markdown; the inner `---` is a `FrontmatterValue` content line of the block scalar.

`fm_block_scalars.md`:

```
---
lit: |
  kept
  lines
clip: >
  folded
  text
strip: |-
  no trailing
keep: |+
  trailing kept

next: after-blank
---
```

Unit test for the incremental side, in `incremental.rs`'s test module (or `low_level_tests.rs`, wherever `same_frontmatter_fences` is exercised — grep first): an edit inside the literal block of `fm_fence_inside_block_scalar`-shaped source must NOT be classified as `FrontmatterBoundaryChanged` due to fence mismatch, and must not produce a tree different from a full parse (the existing full-vs-incremental harness in `tests/properties.rs`/`markdown_incremental.rs` style).

- [ ] **Step 2: Run to verify failures**

Run: `cargo test -p waml-syntax` — Expected: classifier test fails (region ends at the inner `---`).

- [ ] **Step 3: Implement the classifier**

Extract the close-fence search from `classify_frontmatter` (~line 105) into `frontmatter_close_fence_line`. New logic: walk lines; maintain `skip_deeper_than: Option<usize>` — when a line (at indent `i`, outside a skip) ends its value position with a block scalar header (`:` then optional space then `|`/`>` + optional chomp/indent digit, then EOL or a comment), set `skip_deeper_than = Some(i)`; while set, a line whose indent `> i` (or blank) is consumed as content and NEVER tested by `is_fence_line`; the first line with indent `<= i` clears the skip and is tested normally. The header detection here is a cheap line-shape scan (no tree needed): significant text matches `.*:\s*[|>][+-]?[0-9]?\s*(#.*)?$` — implement as a small fn `line_opens_block_scalar(content: &str) -> bool` shared with the builder, not a duplicated regex.

`classify_frontmatter` and `incremental.rs::frontmatter_fences` both call `frontmatter_close_fence_line` (export it through `crate::shell` beside `frontmatter_range`, mirroring the existing pub(crate) re-export at `shell.rs:50`). In `frontmatter_fences`, replace the `rfind('\n')`-based close detection with: close = the fence line the shared scan finds (mapped to a `TextRange`); keep the open-fence check as is.

- [ ] **Step 4: Implement the builder side**

In the Task 4 value scan (rule 6): when the value position matches `line_opens_block_scalar`, emit `FrontmatterBlockScalarHeaderToken` for the `|`/`>`+modifiers run (comment after it per the comment rule). The entry enters BLOCK SCALAR state carrying the header's parent indent; subsequent lines with greater indent (or blank) are consumed as `FrontmatterValue` tokens, one per line, full line to `significant_end`, indentation as leading trivia — `#` is CONTENT here, never a comment. First line at indent <= parent closes the scalar (no node wrapper needed: header token + value tokens inside the same `FrontmatterEntry`). Detected content indent = indent of the first non-blank content line (or the explicit digit); Task 7 uses it for stripping — the TREE keeps every byte regardless.

- [ ] **Step 5: Bless goldens, run the gate**

Same golden discipline as Task 4 (dump, read, verify, bless). Then `cargo test --workspace` — Expected: PASS, including the incremental test from Step 1.

- [ ] **Step 6: Commit**

```bash
git add crates/waml-syntax
git commit -m "feat(waml-syntax): block scalars with a block-scalar-aware region scan

The classifier no longer takes a --- inside a | or > block as the close
fence: the close-fence search skips lines more indented than the block's
parent. The same scan now backs same_frontmatter_fences, so an edit inside
a literal block compares real fences on both sides instead of taking a
needless (or wrong) reparse path. The builder emits the header token and
per-line content tokens; # inside a block scalar is content."
```

---

### Task 6: Incremental — indent edits force a full reparse; full-vs-incremental property

Re-indenting restructures the tree without moving the region; the range-and-fence check would wave it through. Frontmatter is cheap: any edit touching leading whitespace inside the frontmatter window forces a full reparse.

**Files:**
- Modify: `crates/waml-syntax/src/incremental.rs` (the check block at ~line 827)
- Modify: `crates/waml-syntax/tests/properties.rs` (extend the full-vs-incremental property)

**Interfaces:**
- Consumes: `frontmatter_range` (already computed at line 827), `ChangeMap::segments()`.
- Produces: edits overlapping the leading-whitespace run of any frontmatter line → `FullReparseReason::FrontmatterBoundaryChanged`.

- [ ] **Step 1: Write the failing property extension**

In `tests/properties.rs`, find the existing edit-then-compare property (the one guarding reparse-vs-full agreement, ~line 536 proptest block) and add a frontmatter-focused case: generate a document with nested frontmatter (embed a fixed template with a nested map + block sequence + block scalar, splice in generated keys/values from `[a-z]{1,6}` / scalar strategies), generate an edit at an arbitrary offset WITHIN the frontmatter range (insert/delete of a small string drawn from `[" ", "  ", "x", ":", "-", "\n", "#c", "\t"]`), apply incrementally and fully, and `prop_assert_eq!` the two trees' `render_to_string` AND their golden/debug structure (use whatever tree-equality the existing property asserts — mirror it exactly). Also a named unit test: inserting two spaces before `  owner: platform` in the nested fixture yields `ReparseOutcome` full with `FullReparseReason::FrontmatterBoundaryChanged`.

- [ ] **Step 2: Run to verify the unit test fails**

Run: `cargo test -p waml-syntax --test properties` — Expected: the named indent test FAILS (incremental path taken); the property may also find divergence.

- [ ] **Step 3: Implement**

At `incremental.rs` ~line 831, extend the frontmatter condition:

```rust
if !same_optional_range(old_frontmatter, new_frontmatter, &map)
    || !same_frontmatter_fences(&old, &new_text, old_frontmatter, new_frontmatter, &map)
    || edit_touches_frontmatter_leading_whitespace(&new_text, new_frontmatter, &map)
{
    return full(FullReparseReason::FrontmatterBoundaryChanged);
}
```

```rust
/// Any change segment overlapping the frontmatter window whose surrounding
/// new-text line has the segment inside (or adjacent to) its leading
/// whitespace forces a full reparse: indentation restructures the tree
/// without moving the region. Frontmatter is small; precision buys nothing.
fn edit_touches_frontmatter_leading_whitespace(
    new: &SourceText,
    frontmatter: Option<TextRange>,
    map: &ChangeMap,
) -> bool {
    let Some(frontmatter) = frontmatter else { return false };
    map.segments().iter().any(|segment| {
        let range = segment.new;
        if range.end() <= frontmatter.start() || frontmatter.end() <= range.start() {
            return false;
        }
        // Conservative: if the segment starts at or before the first
        // non-whitespace column of its line in the NEW text, indentation
        // may have changed.
        let source = new.shared();
        let start = range.start().to_usize().min(source.len());
        let line_start = source[..start].rfind('\n').map_or(0, |at| at + 1);
        let content_start = line_start
            + source[line_start..].bytes().take_while(|b| *b == b' ' || *b == b'\t').count();
        start <= content_start
    })
}
```

If in practice this makes essentially every frontmatter edit a full reparse — acceptable and spec-endorsed; do NOT add cleverness to avoid it.

- [ ] **Step 4: Gate and commit**

Run: `cargo test --workspace` — Expected: PASS.

```bash
git add crates/waml-syntax
git commit -m "fix(waml-syntax): treat frontmatter indent edits as boundary changes

Re-indenting restructures the nested frontmatter tree without moving the
region or its fences, which the range-and-fence check waved through. Any
edit touching a line's leading whitespace inside the frontmatter window now
forces a full reparse. Extends the full-vs-incremental property with
frontmatter-interior edits including whitespace and tabs."
```

---

### Task 7: Model — FmValue::Map/Null, 1.2 scalars via the shared classifier, nested reading

**Files:**
- Modify: `crates/waml/src/frontmatter.rs` (`FmValue`, seed visitor, `parse_value`, `decode_quoted_string`, `parse_closed_syntax`)
- Modify: `crates/waml-syntax/src/lib.rs` only if the Task 3 re-export needs widening (it should not)

**Interfaces:**
- Consumes: `waml_syntax::{classify_bare_scalar, FrontmatterScalarKind}` (Task 3); Task 4/5 tree shape.
- Produces (Task 8 renders these):

```rust
pub enum FmValue {
    Null,                       // null, ~, empty value
    Str(String),
    Bool(bool),
    Num(f64),
    List(Vec<FmValue>),
    Map(Vec<(String, FmValue)>),  // insertion-ordered, like Frontmatter::entries
}
```

serde: the seed visitor gains `visit_unit`/`visit_none` → `Null` and `visit_map` → `Map` (depth-capped like `visit_seq` — `MAX_VALUE_DEPTH` applies to maps too). NOTE the spec's "Null before Str in the untagged derive" concern is discharged by the existing MANUAL visitor: a JSON `null` hits `visit_unit`, never `visit_str` — add a test proving it.

- [ ] **Step 1: Write failing tests**

In the tests module:

```rust
#[test]
fn parse_value_follows_yaml_12_core() {
    assert_eq!(parse_value("null"), FmValue::Null);
    assert_eq!(parse_value("~"), FmValue::Null);
    assert_eq!(parse_value(""), FmValue::Null);
    assert_eq!(parse_value("NO"), FmValue::Str("NO".into()));
    assert_eq!(parse_value("yes"), FmValue::Str("yes".into()));
    assert_eq!(parse_value("0x1A"), FmValue::Num(26.0));
    assert_eq!(parse_value("0o17"), FmValue::Num(15.0));
    assert_eq!(parse_value(".inf"), FmValue::Num(f64::INFINITY));
    assert_eq!(parse_value("-.inf"), FmValue::Num(f64::NEG_INFINITY));
    assert!(matches!(parse_value(".nan"), FmValue::Num(n) if n.is_nan()));
    assert_eq!(parse_value("6.02e23"), FmValue::Num(6.02e23));
    assert_eq!(parse_value("2026-08-04"), FmValue::Str("2026-08-04".into()));
    // Quoted is ALWAYS Str:
    assert_eq!(parse_value("\"true\""), FmValue::Str("true".into()));
    assert_eq!(parse_value("'it''s'"), FmValue::Str("it's".into()));
    // Nested flow with quote-aware, nesting-aware splitting:
    assert_eq!(
        parse_value("[a, [b, c], \"x,y\"]"),
        FmValue::List(vec![
            FmValue::Str("a".into()),
            FmValue::List(vec![FmValue::Str("b".into()), FmValue::Str("c".into())]),
            FmValue::Str("x,y".into()),
        ])
    );
}

#[test]
fn double_quote_escapes_gain_tab_nul_unicode() {
    assert_eq!(parse_value("\"a\\tb\\0c\\u00e9\""), FmValue::Str("a\tb\0cé".into()));
}

#[test]
fn nested_frontmatter_reads_into_maps_and_lists() {
    let source = "---\nmeta:\n  owner: platform\n  n: 3\nauthors:\n  - name: Ana\n    team: platform\nstereotype:\n  - aggregateRoot\n  - entity\ndup: 1\ndup: 2\ndesc: |\n  line one\n  line two\nfolded: >\n  a\n  b\nempty:\n---\n";
    // Parse through the same public path okf::shell uses (grep how shell.rs:176
    // obtains the Frontmatter node from a parsed document and mirror it).
    let fm = parse_frontmatter_for_test(source);
    assert_eq!(
        fm.get("meta"),
        Some(&FmValue::Map(vec![
            ("owner".into(), FmValue::Str("platform".into())),
            ("n".into(), FmValue::Num(3.0)),
        ]))
    );
    assert_eq!(
        fm.get("authors"),
        Some(&FmValue::List(vec![FmValue::Map(vec![
            ("name".into(), FmValue::Str("Ana".into())),
            ("team".into(), FmValue::Str("platform".into())),
        ])]))
    );
    assert_eq!(fm.get("dup"), Some(&FmValue::Num(2.0)), "last duplicate wins");
    assert_eq!(fm.get("desc"), Some(&FmValue::Str("line one\nline two\n".into())), "| keeps newlines, clip chomping");
    assert_eq!(fm.get("folded"), Some(&FmValue::Str("a b\n".into())), "> folds to spaces");
    assert_eq!(fm.get("empty"), Some(&FmValue::Null));
}

#[cfg(feature = "serde")]
#[test]
fn wire_null_and_map_round_trip() {
    let v: FmValue = serde_json::from_str("null").unwrap();
    assert_eq!(v, FmValue::Null, "JSON null must be Null, not the string \"null\"");
    let v: FmValue = serde_json::from_str("{\"a\": 1, \"b\": null}").unwrap();
    assert_eq!(v, FmValue::Map(vec![("a".into(), FmValue::Num(1.0)), ("b".into(), FmValue::Null)]));
    // Hostile depth via maps is capped like lists:
    let deep = format!("{}1{}", "{\"k\":".repeat(100), "}".repeat(100));
    assert!(serde_json::from_str::<FmValue>(&deep).is_err());
}
```

`parse_frontmatter_for_test` is a small test helper in the module: parse the source with `waml_syntax`'s document parse, locate the `Frontmatter` node exactly the way `crates/waml/src/okf/shell.rs:176` does, and call `parse_closed_syntax`.

- [ ] **Step 2: Run to verify failures**

Run: `cargo test -p waml frontmatter` — Expected: FAIL/compile errors (`Null`, `Map` missing).

- [ ] **Step 3: Implement**

1. Add `Null` and `Map` variants; extend `PartialEq` (Null == Null; Map compares entry vectors). Extend the serde `Serialize` derive coverage: `Null` serializes as unit (`serializer.serialize_unit()` — with `untagged`-style derive on `Serialize` this needs `Map` to serialize as a map preserving order: add a manual `Serialize` arm or a small newtype mirroring the `Frontmatter` impl at line 52). Extend the seed visitor with `visit_unit`, `visit_none`, and `visit_map` (depth-capped, `next_entry_seed` with `FmValueSeed { depth: depth + 1 }` for values).
2. `parse_value`: bare path routes through `waml_syntax::classify_bare_scalar`; delete the local `NUM_RE`-based bool/num logic (keep `NUM_RE` only if the writer still needs it until Task 8 — prefer moving all numeric checks to the classifier now and deleting `NUM_RE`). Number conversion: `0x`/`0o` via `i64::from_str_radix(&s[2..], 16|8)` (on overflow → `Str`, never panic), `.inf`/`.nan` mapped explicitly, else `f64::parse` (failure → `Str`). Flow `[…]`: replace the naive `split(',')` with a quote-aware, bracket-depth-aware splitter (single fn `split_flow_items(&str) -> Vec<&str>` walking chars, tracking `in_single`/`in_double`/`depth`); items are `parse_value_at(item.trim(), depth+1)`; keep the `MAX_VALUE_DEPTH` cap and the existing empty-item filter EXCEPT that a genuinely empty flow `[]` is `List(vec![])`. Single-quoted: strip quotes, `''` → `'`, always `Str`. Double-quoted: `decode_quoted_string` gains `\t`, `\0`, `\uXXXX` (invalid `\u` sequence: keep verbatim, matching the existing unknown-escape behaviour; the `InvalidEscapeSequence` DIAGNOSTIC is emitted by the parser side — add the `InvalidEscapeSequence` variant to `OkfSyntaxDiagnosticCode` in `kind.rs` now and emit it in the builder where a `FrontmatterQuotedValueToken`'s content contains an unknown or malformed escape; one small parser test for it).
3. `parse_closed_syntax`: full recursive walk. `fn value_of_entry(entry: &SyntaxNode<...>) -> Option<FmValue>`: nested `FrontmatterMapping` child → `Map` (recurse entries, skip incomplete ones — existing behaviour, duplicate keys: last wins at each level); `FrontmatterSequence` child → `List` (each `FrontmatterSequenceItem`'s value the same way); `FrontmatterBlockScalarHeaderToken` present → assemble from the entry's `FrontmatterValue` content tokens: strip the detected indent (indent of first non-blank content line, or the explicit digit + parent indent), `|` joins with `\n`, `>` folds single newlines to spaces keeping blank lines as `\n`, chomping: default clip = single trailing `\n`, `-` strips all, `+` keeps all → `Str`; `FrontmatterQuotedValueToken` → decoded `Str`; missing/absent value token → `Null`; bare `FrontmatterValue` → `parse_value`. Depth-cap the recursion with `MAX_VALUE_DEPTH` (hostile deep nesting in the tree must not overflow — return `None` for the entry past the cap). No unwraps.
4. Extend the Task 2 strategy's leaf set with `Just(FmValue::Null)` — the round-trip proptest MUST now fail if the writer doesn't handle `Null`… but the writer changes are Task 8. To keep this task green: add `Null`/`Map` rendering as the MINIMAL correct forms in `render_value_at` right here (Null → empty string in entry position is Task 8's business; for now render `Null` as `null`?). STOP — do not half-implement the writer here. Instead: do NOT touch the strategy in this task; `render_value_at` gets exhaustive-match arms that keep compiling (`FmValue::Null => "null".to_string()` and `FmValue::Map(_) => unreachable-by-strategy` is a placeholder that would violate the no-panic rule). Correct resolution: give `render_value_at` real, simple arms now — `Null => "null".to_string()`, `Map(entries) => temporary flow-less fallback rendering as a quoted-empty is WRONG`. The honest minimal arm that satisfies "no panic, no corruption": render `Map` via the SAME block renderer Task 8 specifies — which means the small `render_entry` function moves INTO this task if the compiler forces exhaustiveness. Decision: implement `render_value_at`'s two new arms as `Null => "null".into()` and `Map => render_map_flowless(entries)` where `render_map_flowless` produces the Task 8 block form (it is ~15 lines; see Task 8 Step 3 for the exact code) — Task 8 then owns `scalar_needs_quote`, sequence-of-maps, and the proptest extension. Update the Task 2 strategy for `Null` only (`Map` strategy arrives in Task 8 with the entry-level renderer test).
5. Scan in-repo fixtures and seeds for documents carrying a trailing `# comment` inside a frontmatter value (`grep -rn "#" --include="*.md" crates/*/tests/fixtures fuzz/seeds | grep -B0 ":"` then eyeball): the spec says comment stripping changing those values is the fix working — list any affected fixture in the commit body.

- [ ] **Step 4: Gate and commit**

Run: `cargo test --workspace` — Expected: PASS, including the Task 2 proptest with `Null` in the strategy.

```bash
git add crates/waml crates/waml-syntax
git commit -m "feat(waml): FmValue gains Map and Null; scalars follow YAML 1.2 core

parse_value now routes bare scalars through waml-syntax's shared
classifier: null/~/empty are Null, NO/yes/on stay strings, hex/octal/
inf/nan parse as numbers, date-shaped stays a string. Flow sequences split
quote- and nesting-aware so [a, [b, c]] and comma-bearing quoted items
survive. Both quote styles decode (single with '' escape; double gains
\\t, \\0, \\uXXXX). parse_closed_syntax walks the nested tree: maps,
sequences of maps, block scalars with folding and chomping, last duplicate
key wins. The wire form gains unit->Null and map->Map, depth-capped."
```

---

### Task 8: Writer — 1.2-core quoting, block mappings, flow sequences, proptest extended to Map/Null

The proptest from Task 2 (already covering `Null`) is extended to `Map` FIRST, red, then the writer makes it green. This is the load-bearing task; nothing ships if `parse_value(render_value(v)) != v`.

**Files:**
- Modify: `crates/waml/src/frontmatter.rs` (`scalar_needs_quote`, `render_value_at`, `render_frontmatter`, new `render_entry`; tests)

**Interfaces:**
- Consumes: `classify_bare_scalar` (Task 3), `FmValue::{Map,Null}` (Task 7).
- Produces: `render_frontmatter(&Frontmatter) -> String` unchanged in signature; nested maps render as two-space-indented block mappings; sequences render flow `[a, b]` / `[]` EXCEPT a sequence containing any `Map` item, which renders as a block sequence; strings with newlines stay double-quoted `\n`-escaped (never `|`); block scalars are never emitted.

- [ ] **Step 1: Extend the proptest strategy (failing)**

In `fm_value_strategy`'s `prop_recursive` closure add:

```rust
prop::collection::vec(("[a-z][a-z0-9_]{0,8}", inner.clone()), 0..4).prop_map(FmValue::Map),
```

And change the round-trip property to go through the ENTRY level for `Map` values (a nested map is only expressible as a block form under a key, not as a standalone value string): rework `rendered_frontmatter_entries_reparse` into the authoritative property — render the whole `Frontmatter`, then re-parse the rendered text through the full document path (wrap in `---\n…\n---\n`, parse with `waml_syntax`, `parse_closed_syntax`) and `prop_assert_eq!` the resulting `Frontmatter` equals the input. Keep the value-level `rendered_value_reparses_identically` property restricted to non-Map values (filter with `prop_filter` or a strategy without the Map arm). Also add named unit tests:

```rust
#[test]
fn nested_map_renders_as_block_mapping() {
    let fm = Frontmatter { entries: vec![(
        "meta".into(),
        FmValue::Map(vec![
            ("owner".into(), FmValue::Str("platform".into())),
            ("detail".into(), FmValue::Map(vec![("level".into(), FmValue::Num(3.0))])),
        ]),
    )]};
    assert_eq!(render_frontmatter(&fm), "meta:\n  owner: platform\n  detail:\n    level: 3");
}

#[test]
fn sequence_of_maps_renders_as_block_sequence() {
    let fm = Frontmatter { entries: vec![(
        "authors".into(),
        FmValue::List(vec![FmValue::Map(vec![
            ("name".into(), FmValue::Str("Ana".into())),
            ("team".into(), FmValue::Str("platform".into())),
        ])]),
    )]};
    assert_eq!(render_frontmatter(&fm), "authors:\n  - name: Ana\n    team: platform");
}

#[test]
fn existing_documents_render_byte_identical() {
    // The compatibility pin from the spec: scalar lists stay flow.
    let fm = Frontmatter { entries: vec![
        ("type".into(), FmValue::Str("uml.Class".into())),
        ("stereotype".into(), FmValue::List(vec![
            FmValue::Str("aggregateRoot".into()), FmValue::Str("entity".into()),
        ])),
    ]};
    assert_eq!(render_frontmatter(&fm), "type: uml.Class\nstereotype: [aggregateRoot, entity]");
}

#[test]
fn norway_no_longer_needs_quoting_but_new_structurals_do() {
    assert!(!scalar_needs_quote("NO"));
    assert!(!scalar_needs_quote("yes"));
    for s in ["null", "~", ".inf", "-.inf", ".nan", "0x1A", "0o17", "1e3",
              "- item", "#x", "[x", "{a}", "&a", "*a", "!t", "|", ">f", "%v",
              "@a", "`c", "'q", "\"q", "a: b", "a #b", "key:"] {
        assert!(scalar_needs_quote(s), "{s:?} must quote");
    }
    assert!(!scalar_needs_quote("a:b"), "colon without space is safe in YAML");
    assert!(!scalar_needs_quote("2026-08-04"));
}
```

- [ ] **Step 2: Run to verify failures**

Run: `cargo test -p waml frontmatter` — Expected: FAIL (old quote rules, `Map` flow-less fallback formatting mismatch, `null` bare string now reparses as Null → the proptest itself should be red for `Str("null")`-shaped strings if Task 7 left `scalar_needs_quote` untouched — that red is exactly the corruption this task fixes).

- [ ] **Step 3: Implement**

```rust
fn scalar_needs_quote(s: &str) -> bool {
    use waml_syntax::FrontmatterScalarKind;
    if s.is_empty() || s != s.trim() {
        return true;
    }
    // Would reparse as a non-string type (covers true/false, null/~, every
    // numeric form including hex, octal, .inf, .nan, exponents).
    if waml_syntax::classify_bare_scalar(s) != FrontmatterScalarKind::Str {
        return true;
    }
    // Structural first character.
    let first = s.chars().next().expect("non-empty checked above");
    if matches!(first, '#' | '[' | ']' | '{' | '}' | '&' | '*' | '!' | '|' | '>' | '%' | '@' | '`' | '"' | '\'' | ',') {
        return true;
    }
    if s == "-" || s.starts_with("- ") {
        return true;
    }
    // Would open a mapping or a comment mid-scalar, or confuse the writer's
    // own quoted forms / escapes / line structure.
    s.contains(": ") || s.ends_with(':') || s.contains(" #")
        || s.contains('\\') || s.contains('\n') || s.contains('\r') || s.contains('\t')
}
```

(`,` stays in the first-char set and list items with commas now quote properly — extend `render_value_at`'s List arm: an item needing a quote is `format!("\"{}\"", escape_quoted_string(item_str))`, and any item containing `,`, `[`, or `]` must quote so the flow splitter reassembles it — encode that as a second predicate `flow_item_needs_quote(s) = scalar_needs_quote(s) || s.contains(',') || s.contains('[') || s.contains(']')` used only in the List arm.)

`escape_quoted_string` gains `\t` → `\\t` and `\0` → `\\0` and `\r` handling consistent with `normalize_line_endings` (spec keeps normalize-to-`\n` behaviour; keep it).

Entry/value rendering:

```rust
fn render_entry(out: &mut String, key: &str, value: &FmValue, indent: usize) {
    let pad = "  ".repeat(indent);
    match value {
        FmValue::Map(entries) => {
            out.push_str(&format!("{pad}{key}:"));
            if entries.is_empty() {
                // An empty map has no block form; there is no flow map. Render
                // as an explicit empty line-value: the reader maps a valueless
                // key to Null, so an empty Map round-trips as Null — pin this
                // asymmetry with a named test and normalize empty Map to Null
                // at the strategy level (prop_map filtering) rather than
                // pretending it survives.
                out.push('\n');
            } else {
                out.push('\n');
                for (k, v) in entries {
                    render_entry(out, k, v, indent + 1);
                }
            }
        }
        FmValue::List(items) if items.iter().any(|i| matches!(i, FmValue::Map(_))) => {
            out.push_str(&format!("{pad}{key}:\n"));
            for item in items {
                render_block_sequence_item(out, item, indent + 1);
            }
        }
        other => {
            out.push_str(&format!("{pad}{key}: {}\n", render_value(other)));
        }
    }
}

fn render_block_sequence_item(out: &mut String, item: &FmValue, indent: usize) {
    let pad = "  ".repeat(indent);
    match item {
        FmValue::Map(entries) => {
            let mut first = true;
            for (k, v) in entries {
                if first {
                    // "- name: Ana" — dash and first entry share the line.
                    match v {
                        FmValue::Map(_) | FmValue::List(_) => {
                            out.push_str(&format!("{pad}- {k}:\n"));
                            // nested container under the dash-key
                            render_nested_under(out, v, indent + 2);
                        }
                        scalar => out.push_str(&format!("{pad}- {k}: {}\n", render_value(scalar))),
                    }
                    first = false;
                } else {
                    render_entry(out, k, v, indent + 1);
                }
            }
            if entries.is_empty() {
                out.push_str(&format!("{pad}-\n"));
            }
        }
        other => out.push_str(&format!("{pad}- {}\n", render_value(other))),
    }
}
```

(`render_nested_under` renders a Map's entries at the given indent, or a List item-by-item — factor so `render_entry` and it share the container logic; keep the whole renderer under ~80 lines, recursion depth-capped at `MAX_VALUE_DEPTH` returning early like `render_value_at` does today.)

`render_frontmatter` builds via `render_entry` for every top-level pair and strips the single trailing `\n` (existing callers — `seed.rs:27`, `uml/format.rs` — expect no trailing newline; verify against their tests). `FmValue::Null` in value position renders as `null` (explicit beats empty: survives whitespace trimming and is unambiguous). `render_value_at` keeps the flow List arm (scalar-only lists) with `flow_item_needs_quote`, and its `Map` arm now defers to the block renderer only via `render_entry` (a bare `render_value(Map)` should be unreachable from `render_frontmatter`; make `render_value_at`'s Map arm render the flowless block form WITHOUT a key by rendering entries at indent 0 — used only by tests/debug paths — and document it).

Empty-Map normalization: since `key:` with no deeper lines reads back as `Null`, add to the proptest strategy a `prop_map` that recursively replaces `Map(vec![])` with `Null` before asserting, with a comment citing this step; add the named asymmetry test `empty_map_renders_and_reparses_as_null`.

- [ ] **Step 4: Run tests then the full gate**

Run: `cargo test -p waml frontmatter` (proptest at 1024 cases must be green), then `cargo test --workspace`. Check `seed`/`share`/`index_md` snapshots for rendering diffs — `stereotype: [aggregateRoot, entity]` must be byte-identical per the spec's compatibility pin.

- [ ] **Step 5: Commit**

```bash
git add crates/waml
git commit -m "feat(waml): writer renders nested values under the 1.2 quoting rules

scalar_needs_quote is rewritten against the shared classifier: NO stops
quoting (no longer a bool), while null/~, hex/octal/inf/nan forms, and
structural leaders (- , #, [, {, &, *, !, |, >, %, @, backtick, quotes)
now quote, as do : -space and space-# interiors. Nested maps render as
two-space block mappings; sequences stay flow with quote-aware items,
except sequences of maps which render as block sequences. Newline-bearing
strings stay double-quoted with escapes; block scalars are never emitted.
The round-trip proptest now covers Map and Null (empty Map normalizes to
Null, pinned by a named test)."
```

---

### Task 9: Presentation — frontmatter token roles and CodeToken coloring

**Files:**
- Modify: `crates/waml-syntax/src/markdown/snapshot.rs` (`MarkdownSemanticRole`, `semantic_role`, `source_role`)
- Modify: `crates/waml-markdown-editor/src/presentation/compile.rs` (`marker_role` ~line 220, `content_role` ~line 269)
- Test: beside the existing role-mapping tests in the editor crate (grep `TextRole::CodeToken` in tests for the pattern)

**Interfaces:**
- Consumes: Task 4/5 token kinds; `classify_bare_scalar` (Task 3); `CodeTokenRole` (`highlight.rs:20`: Keyword, Type, Property, String, Number, Comment, Punctuation, Invalid); Task 1's Code block.
- Produces: `MarkdownSemanticRole` gains `FrontmatterKey`, `FrontmatterPunctuation` (colon, dash, brackets — check whether the fence should be distinct: yes, keep `FrontmatterFence` too), `FrontmatterComment`, `FrontmatterScalar`, `FrontmatterFence`; keep the existing `Frontmatter` variant for the node-level spans (Document-level consumers may match on it — grep `MarkdownSemanticRole::Frontmatter` across the workspace and update every site).

- [ ] **Step 1: Write the failing mapping test**

In the editor's presentation tests: compile `"---\ntitle: Order # note\nn: 3\nok: true\nbad\nstereotype: [a, b]\n---\n"` and assert per-range `TextRole`s:
- `title` → `TextRole::CodeToken(CodeTokenRole::Property)`
- `:` and `[`/`]`/`,` and `-` (add a seq fixture line) → `CodeToken(Punctuation)`
- `# note` → `CodeToken(Comment)`
- `Order` → `CodeToken(String)`; `3` → `CodeToken(Number)`; `true` → `CodeToken(Keyword)`
- `bad` (BadToken line) → `CodeToken(Invalid)`
- fences `---` → `CodeToken(Punctuation)`
Follow the harness style of the nearest existing compile test; assert by looking up the run covering each substring's range.

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p waml-markdown-editor` — Expected: FAIL (everything is `TextRole::Frontmatter`).

- [ ] **Step 3: Implement**

`snapshot.rs::semantic_role` — token-kind-precise mapping (the fn takes the KIND; check whether tokens flow through `semantic_role` or only nodes do — if spans inherit the owning node's role, extend the span-construction site so token kinds override, mirroring how code-fence tokens get distinct roles; find that mechanism first by tracing how `MarkdownSyntaxSpan` gets `semantic_role` for `CodeFenceToken`):

```rust
K::FrontmatterKey /* hidden token kind */ | K::FrontmatterKeyToken => MarkdownSemanticRole::FrontmatterKey,
K::ColonToken /* only inside frontmatter — colon is frontmatter-only today; verify by grep */
    | K::FrontmatterDashToken => MarkdownSemanticRole::FrontmatterPunctuation,
K::FrontmatterOpenFence | K::FrontmatterCloseFence | K::FrontmatterFenceToken => MarkdownSemanticRole::FrontmatterFence,
K::FrontmatterCommentToken => MarkdownSemanticRole::FrontmatterComment,
K::FrontmatterValue | K::FrontmatterValueToken | K::FrontmatterQuotedValueToken
    | K::FrontmatterBlockScalarHeaderToken => MarkdownSemanticRole::FrontmatterScalar,
```

`compile.rs` — in BOTH `marker_role` and `content_role`, replace the single `MarkdownSemanticRole::Frontmatter => TextRole::Frontmatter` arm with:

```rust
MarkdownSemanticRole::Frontmatter => TextRole::Frontmatter, // node-level fallback
MarkdownSemanticRole::FrontmatterKey => TextRole::CodeToken(CodeTokenRole::Property),
MarkdownSemanticRole::FrontmatterPunctuation | MarkdownSemanticRole::FrontmatterFence =>
    TextRole::CodeToken(CodeTokenRole::Punctuation),
MarkdownSemanticRole::FrontmatterComment => TextRole::CodeToken(CodeTokenRole::Comment),
MarkdownSemanticRole::FrontmatterScalar => frontmatter_scalar_role(span, text),
```

```rust
/// The color is the model's verdict: classification comes from the same
/// waml-syntax classifier parse_value uses, so a value painted Number can
/// never be read as a Str.
fn frontmatter_scalar_role(span: &MarkdownSyntaxSpan, text: &SourceText) -> TextRole {
    let Ok(slice) = text.slice(span.range) else {
        return TextRole::CodeToken(CodeTokenRole::String);
    };
    let raw = slice.write_to_string();
    let s = raw.trim();
    if s.starts_with('"') || s.starts_with('\'') || s.starts_with('|') || s.starts_with('>') {
        return TextRole::CodeToken(CodeTokenRole::String);
    }
    if s.starts_with('[') {
        // Flow sequences arrive as one token; paint them String-family? No:
        // brackets punctuate. If splitting the token is out of reach here,
        // paint the whole run String — pick ONE and pin it in the test.
        return TextRole::CodeToken(CodeTokenRole::String);
    }
    match waml_syntax::classify_bare_scalar(s) {
        waml_syntax::FrontmatterScalarKind::Bool | waml_syntax::FrontmatterScalarKind::Null =>
            TextRole::CodeToken(CodeTokenRole::Keyword),
        waml_syntax::FrontmatterScalarKind::Number => TextRole::CodeToken(CodeTokenRole::Number),
        waml_syntax::FrontmatterScalarKind::Str => TextRole::CodeToken(CodeTokenRole::String),
    }
}
```

`BadToken` inside frontmatter: check what role BadToken currently maps to (`MarkdownSemanticRole::Recovery` → `TextRole::Recovery` per compile.rs line 273); the spec wants `Invalid` — add: in the frontmatter path, `Recovery` spans whose owner is inside a `Frontmatter` node map to `TextRole::CodeToken(CodeTokenRole::Invalid)` (use `span.owner` + the snapshot queries to test ancestry, the way `destination_range` does at line 230; if ancestry is not cheaply reachable, map ALL of frontmatter's BadTokens at the snapshot level with a distinct `FrontmatterInvalid` semantic role instead — choose whichever the snapshot structure supports and pin it in the test).

Style check: `TextRole::CodeToken(_)` already carries mono metrics (`style.rs:132`) — nothing to add. `TextRole::Frontmatter` remains for node-level/whitespace fallback runs.

- [ ] **Step 4: Gate and commit**

Run: `cargo test --workspace` — Expected: PASS.

```bash
git add crates/waml-syntax crates/waml-markdown-editor
git commit -m "feat(presentation): syntax-color frontmatter from its own tree

MarkdownSemanticRole gains frontmatter token roles (key, punctuation,
fence, comment, scalar) so MarkdownSyntaxSpan stops flattening the region
to one role. compile.rs maps them onto the CodeToken palette; bare scalars
are classified by the same waml-syntax classifier the model uses, so the
color is the model's verdict. Bad lines paint Invalid. No highlighter host
involved."
```

Visual note for the reviewer: the gate cannot assert pixels. After this task, run the native editor (`/run` skill, any preset with frontmatter) and confirm: code pitch (no crazy line spacing), colored keys/values, fences visible with `hide_syntax` on. State the verification in the task's completion report.

---

### Task 10: Fuzz seeds and the conformance sweep

**Files:**
- Create: `fuzz/seeds/parse_write/frontmatter-nested.md`, `fuzz/seeds/parse_write/frontmatter-block-scalars.md`, `fuzz/seeds/parse_write/frontmatter-hostile.md`
- Modify (only if the sweep finds gaps): fixtures under `crates/waml-syntax/tests/fixtures/shell/`

**Interfaces:** none produced; consumes everything above.

- [ ] **Step 1: Write the seeds**

`frontmatter-nested.md`:

```
---
# comment
'k1': 'it''s'
"k2": "a\tb\u00e9"
meta:
  a:
    - 1
    - x: y
seq: [a, [b, "c,d"], 0x1F, .inf]
dup: 1
dup: 2
empty:
tilde: ~
---

Body.
```

`frontmatter-block-scalars.md`:

```
---
a: |
  ---
  # not a comment
b: >-
  folded
  text

  after blank
c: |+
  kept

---
```

`frontmatter-hostile.md` (mis-indents, tabs, unterminated quote, colon soup, deep nesting):

```
---
a: b: c
	tab: indent
   odd:
  dedent-mismatch: x
q: "never closed
deep: [[[[[[[[1]]]]]]]]
-: bare
: nokey
---
```

- [ ] **Step 2: Run the fuzz target briefly over the seeds**

Check how the fuzz harness runs (`ls fuzz`, `cat fuzz/Cargo.toml` / README): if `cargo fuzz` is set up, run `cargo fuzz run parse_write -- -runs=100000 -max_total_time=60` from `fuzz/`; if the Windows host cannot run libFuzzer, at MINIMUM add a plain `#[test]` in `waml-syntax` that feeds each seed file through the full parse + `render_to_string` identity + `parse_closed_syntax` and asserts no panic. Do not skip both.

- [ ] **Step 3: Conformance sweep against the spec**

Re-read the spec's "Valid YAML this rejects or mangles today" and "Accepted here but read differently" lists; for each of the 8 bullets confirm a test now pins the new behaviour (`# a comment` → comment token not BadToken; nested maps/sequences/multiline parse; `'single quoted'` unwraps; trailing comment strips; `null`/`~` → Null; `a: b: c` → malformed; `[a, [b, c]]` nests; `yes`/`on` stay strings). Add any missing case as a small named test where its neighbours live. Verify no in-repo `.md` fixture outside the frontmatter tests changed meaning unintentionally: `git grep -l "^---" -- "*.md"` limited to `crates/` and diff test outcomes.

- [ ] **Step 4: Full gate and commit**

Run: `cargo test --workspace` — Expected: PASS.

```bash
git add fuzz/seeds/parse_write crates/waml-syntax crates/waml
git commit -m "test(frontmatter): fuzz seeds and conformance sweep for YAML alignment

Adds nesting, block-scalar, and hostile-input seeds to parse_write, plus a
no-panic seed test, and closes the conformance checklist from the spec:
every previously mangled or misread construct now has a named test pinning
the new behaviour."
```

---

## Self-Review (completed during planning)

- **Spec coverage:** §1 tree/scanner → Tasks 4–5; §2 scalars/value model → Tasks 3, 7; §3 writer/round-trip → Tasks 2, 8; §4 incremental/shell → Tasks 5 (fences), 6 (indent); §5 presentation → Tasks 1 (block/visibility), 9 (roles/colors), shared classifier consumption in 9; §6 errors → diagnostics distributed: Task 4 (Duplicate, Tab, InvalidIndent, Unterminated), Task 7 (InvalidEscapeSequence); Testing list 1→Task 2/8, 2→Task 6, 3→Tasks 4/5 fixtures, 4→Task 3, 5→Task 10, 6→Tasks 1/9. Non-goals respected: no flow maps, no 1.1 bools, no date type, no path lookup, no lowering, no HighlighterRegistry wiring.
- **Risk ordering:** round-trip proptest (Task 2) precedes every writer change (Task 8; Task 7's minimal Map/Null arms are covered by extending the strategy with Null in the same task). Classifier/region risk carries its named fixture (Task 5 Step 1).
- **Type consistency:** `classify_bare_scalar`/`FrontmatterScalarKind` used identically in Tasks 3, 7, 8, 9; token kind names identical across Tasks 4, 5, 9; `render_entry` signature consistent between Tasks 7 and 8.
- **Known judgment calls the implementer may adjust WITH a pinning test:** flow-sequence token painting (Task 9), empty-Map→Null normalization (Task 8), `Null` rendering as `null` vs empty (Task 8).
