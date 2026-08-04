# Frontmatter YAML Alignment — Design

**Date:** 2026-08-04
**Status:** Approved, ready for implementation planning

## Problem

waml's frontmatter looks like YAML, is called YAML by every tool that inspired
it, and is not YAML. Nothing in the repository parses YAML. What exists is a
hand-rolled line splitter (`waml-syntax/src/markdown/parser.rs`) that splits each
line at its first `:`, plus a five-rule value classifier
(`waml/src/frontmatter.rs::parse_value`).

That divergence costs us in both directions.

Valid YAML this rejects or mangles today:

- `# a comment` — no colon, so it becomes a `BadToken` and flags the whole block
  `FrontmatterNotClean`
- nested maps, block sequences (`- item`), multiline scalars (`|`, `>`) — every
  line is judged alone, so `- item` is a malformed entry
- `'single quoted'` — quotes are kept, yielding the literal string `'single quoted'`
- `key: value # trailing comment` — the comment becomes part of the value, silently
- `null` / `~` — become the strings `"null"` / `"~"`

Accepted here but read differently by YAML:

- `a: b: c` — splits at the first colon, value is `b: c`; YAML errors
- `[a, [b, c]]` — `parse_value` splits on every comma at one level, so nesting flattens
- unquoted `yes` / `on` — plain strings here; YAML 1.1 makes them booleans

The trailing-comment case is the worst of these: it parses clean and means the
wrong thing.

Separately, frontmatter is the only region of a markdown document with no
`PresentationBlock` of its own. `block_kind` (`waml-markdown-editor/src/presentation/compile.rs`)
has no arm for `Kind::Frontmatter`, so its runs fall to the document root and are
rebuilt by `fragment_parent_owned_runs` into a single paragraph whose row pitch
comes from a `line_spacing_scale` borrowed from whichever span shaped last. That
is the visible "crazy line spacing". Its tokens are also flattened to one
`TextRole::Frontmatter`, so it renders as undifferentiated muted monospace with
no syntax coloring.

## Goals

Parse the YAML that people actually write, without adopting YAML's legacy
misfeatures. Make the editor render frontmatter the way a code editor renders
code. Keep the model's blast radius small.

## Non-Goals

- YAML 1.1 compatibility. `NO` must not become `false`.
- Flow mappings (`{a: 1}`). Flow sequences (`[a, b]`) stay; flow maps are out.
- Anchors, aliases, merge keys, tags, directives, multiple documents.
- A date type. Date-shaped scalars stay strings.
- Nested frontmatter reaching `okf` / `uml` lowering. Not in this change.
- Wiring the `HighlighterRegistry` seam. It is built, validated, tested, and
  unwired; fenced code is unhighlighted in the running editor. Separate work.

## Decisions

Recorded with the reasoning, because each was a live fork during design.

**Scope: Tier B — block structure.** Arbitrary-depth nesting, block sequences
including sequences of maps, and multiline scalars. Rejected: Tier A
(within-the-line fixes only) as too small to stop the churn happening twice;
Tier C (delegate to a YAML crate) as buying spec completeness we explicitly do
not want, at the cost of a dependency, span remapping, and post-filtering the
misfeatures back out.

**Model reach: preserve and render only.** `FmValue` gains `Map`; nested data
round-trips and displays, but there is no path-lookup API and no lowering reads
it. Keeps `lower.rs`, `okf`, and `uml` entirely out of the change. Path lookup
was rejected for now because it invites consumers to couple to nested shapes
before we know which shapes matter.

**Parser architecture: nested nodes, indentation stays trivia (Approach A).**
The green/red tree mirrors the data. Rejected: a flat entry list with a depth
field, because it makes the tree lie about the document, and the editor colors
and hit-tests from that tree; also because the line-per-entry invariant it
protects is voided by multiline scalars anyway. Rejected: a `WamlLanguageIsland`-style
embedded sub-parser, because islands earn their cost when the embedded language
is genuinely foreign, and this one shares the same trivia model and factory.

**Scalars: YAML 1.2 core schema.** This is the spec's own modern answer to the
misfeature we are avoiding.

**Sequences render flow.** `[a, b]` is familiar and can express the empty list.
A proper recursive parser makes flow parsing quote-aware and nesting-aware, so
comma-bearing items and nested lists — unrepresentable today — round-trip. The
one forced exception is a sequence whose item is a map, which has no flow form
once flow maps are out of scope.

**Block scalars are parsed but never emitted.** We accept `|` and `>` because
people write them; the writer emits double-quoted strings with escapes, which
round-trip trivially. Asymmetric on purpose.

## Design

### 1. Tree shape and the scanner

New node kinds: `FrontmatterMapping`, `FrontmatterSequence`,
`FrontmatterSequenceItem`. `FrontmatterEntry` survives but changes meaning — its
value child may be a scalar token or a nested `FrontmatterMapping` /
`FrontmatterSequence`. `Frontmatter` holds exactly one `FrontmatterMapping`
between its fences.

New token kinds: `FrontmatterDashToken` (`-`), `FrontmatterBlockScalarHeaderToken`
(`|`, `>`, with chomping indicators `|-`, `>+`), `FrontmatterQuotedValueToken`
(both quote styles, so presentation can distinguish a quoted string from a bare
one), `FrontmatterCommentToken`. Existing `FrontmatterKeyToken`, `ColonToken`,
`FrontmatterValueToken`, `FrontmatterFenceToken`, `BadToken` keep their roles.

Full fidelity is preserved: every byte stays in the tree and `render_to_string`
reproduces the source exactly.

Scanning keeps the existing two-pass split. The classifier (`frontmatter_class`)
finds the region; the builder (`frontmatter`) walks lines with an indent stack,
opening a mapping or sequence when the column grows and closing on dedent.
Indentation remains leading trivia; columns are computed from token offsets.

**The classifier must become block-scalar-aware.** It currently selects the close
fence as the first line matching `is_fence_line`, with no knowledge of what is
above it. With block scalars that is wrong:

```
---
description: |
  ---
key: value
---
```

The `---` inside the literal block would terminate the frontmatter early and
`key: value` would become markdown. When a line opens a block scalar, the fence
search must skip lines more indented than the block's parent before resuming.
This is a change to region detection, not just to the builder.

A consequence of keeping indentation in trivia: an indentation error — a column
matching no open block — is diagnosed from computed columns rather than falling
out of the grammar. That is how the existing `MalformedFrontmatterEntry` path
already works, but it means indent diagnostics need explicit tests.

### 2. Scalars and the value model

`FmValue` gains `Map(Vec<(String, FmValue)>)` (insertion-ordered, matching
`Frontmatter::entries`) and `Null`. In the serde `untagged` derive, `Null` must
precede `Str`, or a JSON `null` deserializes as the string `"null"`.

Bare scalar typing, YAML 1.2 core:

| Input | Result |
| --- | --- |
| `true`, `false` | `Bool` |
| `NO`, `no`, `yes`, `on`, `off`, `y`, `n` | `Str` |
| `null`, `~`, empty value | `Null` |
| `-?[0-9]+`, `0x…`, `0o…` | `Num` |
| float with optional exponent | `Num` |
| `.inf`, `-.inf`, `.nan` | `Num` |
| date-shaped (`2026-08-04`) | `Str` |
| anything else | `Str` |

Quoting. Single-quoted is literal with `''` as its only escape. Double-quoted
keeps the existing escape set (`\\`, `\"`, `\n`, `\r`) and gains `\t`, `\0`,
`\uXXXX`. A quoted scalar is always `Str`, so `"true"` is how the literal word is
written.

Comments. `#` opens a comment at line start or when preceded by whitespace,
outside quotes and outside block scalars. Inside a block scalar it is content.

Block scalars. `|` keeps newlines; `>` folds them to spaces with blank lines
preserved as breaks. Chomping: default clips to a single trailing newline, `-`
strips, `+` keeps all. Content indentation is stripped relative to the block's
detected indent.

Keys may be bare or quoted. Duplicate keys keep the last value and emit
`DuplicateFrontmatterKey` — silently dropping is how config bugs hide.

Tabs in indentation are an error with their own diagnostic, per spec.

### 3. Writer and the round-trip contract

The invariant: **`parse_value(render_value(v)) == v` for every `FmValue`**,
nested ones included. This is why `scalar_needs_quote` exists, and every new
value form must preserve it or the writer corrupts documents on save.

`scalar_needs_quote` is rewritten against the 1.2 core rules. A bare string needs
quoting when it would parse back as something else: `true`/`false`, `null`/`~`,
any numeric form including `.inf`/`.nan`, empty or whitespace-padded, starting
with a structural character (`- `, `#`, `[`, `{`, `&`, `*`, `!`, `|`, `>`, `%`,
`@`, `` ` ``, a quote), or containing `: ` or ` #`. Note this shrinks in one
place: `NO` no longer needs quoting, because it is no longer a bool.

Nested maps render as block mappings, two-space indent per level, recursively.

Strings containing newlines stay double-quoted with `\n` escapes rather than
becoming `|` blocks — escapes round-trip trivially, block scalars require exact
indent detection and chomping.

Sequences render flow: `[a, b]`, empty as `[]`, items quoted when they would
otherwise parse back differently. The sole exception is a sequence containing a
map, which has no flow form and renders as a block sequence:

```
authors:
  - name: Ana
    team: platform
```

Existing documents are unaffected by the sequence rule — `stereotype: [aggregateRoot, entity]`
stays byte-identical. Comment stripping does change parsed values in documents
that today carry a trailing comment inside a value; that is the fix working, and
it is the only intended churn.

Nothing rewrites on read. Rewrites occur only where a document is written back:
save, `seed`, `share`, `index_md`.

### 4. Incremental reparse and the shell

`incremental.rs` forces a full reparse when the frontmatter range or its fences
change (`FrontmatterBoundaryChanged`); `shell.rs` treats the whole region as one
`ShellWindowKind::Frontmatter` window. That structure survives unchanged and is
what makes this affordable: frontmatter is small and fence-bounded, so the
granularity that matters is "did the region move", not "which entry changed".

Two adjustments:

**Fence detection.** `same_frontmatter_fences` compares old and new fence
positions. Once a `---` inside a block scalar is not a fence, both sides of that
comparison must use the block-scalar-aware scan, or an edit inside a literal
block compares a real fence against a fake one and takes a needless full reparse
— or a wrong incremental one.

**Indent changes are boundary changes.** Re-indenting restructures the tree
without moving the region, which the range-and-fence check would wave through.
Any edit touching leading whitespace inside the frontmatter window triggers a
full reparse. Frontmatter is cheap to reparse; precision here buys nothing and
risks a stale tree.

### 5. Presentation

`block_kind` gains `Kind::Frontmatter => PresentationBlockKind::Code`.
Frontmatter stops being a blockless region rebuilt into a root paragraph and gets
`BlockFlow::Code` — fixed per-line pitch, code padding, the same surface as a
fenced block. This removes the borrowed-`line_spacing_scale` path rather than
tuning a constant.

`Frontmatter` is dropped from `is_syntax_marker`. The editor is an editor: fences
and body are always visible and `hide_syntax` no longer applies to them. Reading
presentation is the separate viewer's concern.

`MarkdownSemanticRole` gains frontmatter token roles — key, colon, dash, fence,
comment, scalar — because `MarkdownSyntaxSpan` is the presentation's only input
and currently flattens all of them. `compile.rs` maps them onto the existing
`CodeToken` palette: key → `Property`; `:`, `-`, brackets → `Punctuation`;
comment → `Comment`; `BadToken` → `Invalid`; scalars by classified type →
`String` / `Number` / `Keyword`.

**Scalar classification lives in `waml-syntax`, beside the tokens, and both
consumers call it** — `waml`'s `parse_value` for the model and `compile.rs` for
the colors. Without this the coloring drifts from the model, and a value painted
as a `Number` could be read as a `Str`. With it, the coloring is the model's
verdict: paste a block sequence and you see it structured; paste something the
format will not honor and it paints `Invalid`.

No highlighter host is required; this comes from our own tree.

### 6. Error handling

Existing diagnostics keep their meaning: `MissingFrontmatterFence`,
`MalformedFrontmatterEntry`, `FrontmatterNotClean`. New codes, each with a real
span rather than the whole block: `DuplicateFrontmatterKey`,
`TabInFrontmatterIndent`, `InvalidFrontmatterIndent`, `UnterminatedQuotedScalar`,
`InvalidEscapeSequence`.

Recovery is per line and never fatal. An unreadable line becomes a `BadToken`
inside its `FrontmatterEntry`; the indent stack keeps its state and later lines
still parse. The region still produces a tree, `FrontmatterNotClean` still marks
it, and the model reads whichever entries came out whole — matching
`parse_closed_syntax`'s existing skip-incomplete behavior.

No `unwrap` reachable from document content. Frontmatter is untrusted input: a
hostile `.waml` file must not take down the LSP or poison the wasm instance.

## Testing

In order of how much trust each carries:

1. **Round-trip property test.** Generate arbitrary `FmValue` trees, `render → parse`,
   assert identity. This is the contract the writer can silently break, so it
   gets a proptest rather than examples.
2. **Full-versus-incremental agreement.** Extend `waml-syntax/tests/properties.rs`
   so an edit inside frontmatter yields the same tree as a full reparse. This is
   the guard on the indent rule, and this class of bug has bitten before.
3. **Conformance fixtures.** Goldens beside `waml-syntax/tests/fixtures/shell/*.golden`,
   one per construct: nested map, block sequence of maps, block sequence of
   scalars, both block scalar styles across all three chomping modes, comments,
   both quote styles, and the `---`-inside-a-block-scalar case.
4. **Scalar classification table.** One test enumerating the 1.2 core rules with
   `NO` / `no` / `yes` / `on` included, so the Norway case is pinned by name.
5. **Fuzz.** Add nesting and block-scalar seeds to `fuzz/seeds/parse_write/`.
6. **Presentation tests.** Token-kind-to-`CodeToken` mapping, plus a layout
   assertion that frontmatter has code pitch rather than paragraph spacing.

Deliberately untested: exact colors (a theme concern), and byte-level goldens of
editor rendering (opaque, and they get blindly re-blessed).

## Risks

- **Round-trip is the load-bearing invariant.** A writer bug corrupts user
  documents on save rather than failing loudly. Mitigated by the proptest, which
  should land before the writer changes.
- **The classifier change touches region detection**, which decides what is
  frontmatter versus markdown. A mistake here misclassifies document bodies. The
  block-scalar fixture is the guard.
- **Comment stripping changes the meaning of existing documents** that carry a
  trailing comment inside a value. Intended, but worth a scan of in-repo
  fixtures during implementation.
