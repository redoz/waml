# Whole-plan fix report

Date: 2026-07-31

## Result

The final fix wave closes the Makepad Markdown authority leak, moves OKF shell
classification to typed Markdown snapshot queries, gives UML one authoritative
syntax tree per Markdown island, and removes the vague `CommonMarkCurrent`
dialect alias.

## RED evidence

- Makepad surface: the mounted Markdown widget contained `"first source"`
  instead of staying empty.
- Makepad alias guard: an aliased widget selected through `ids!(md)` could call
  `set_text` without a violation.
- OKF concept projection: raw scanning included image, escaped, and fenced links,
  and omitted a reference link.
- OKF authored index: expected authored order
  `[real, reference, escaped, fenced, image]`; raw scanning produced a different
  order. Typed queries initially returned no links for tight list items.
- UML: an edited broken Attributes island did not retain the exact sibling
  Values `Arc<SyntaxTree<UmlLanguage>>`.
- Dialect guard: production sources still used `CommonMarkCurrent`.

## Implementation

- `MarkdownSurface` now writes exact source only to `plain_source`. The hidden
  Makepad Markdown compatibility widget stays empty.
- The authority guard follows generic widget aliases and rejects Markdown
  `set_text` ingress. It also rejects OKF raw/regex classifiers and the removed
  dialect alias through in-memory seeds.
- OKF links, citations, H1/title, citation boundary, list membership, and
  authored index order now use `MarkdownSyntaxSnapshot` query records and exact
  ranges. Images, escapes, fenced examples, nested/reference links, citations,
  and index headings are covered.
- Tight list-item text gets inline parsing only for tokens that contain inline
  syntax. Marker whitespace stays structural. CommonMark and GFM rendering stay
  conformant.
- UML stores island snapshots by `(SyntaxIdentity, content_range)`. Each island
  tree owns only its exact section source slice and local ranges. An unchanged
  sibling reuses its exact tree Arc after mapped document edits. The full UML
  tree is compatibility output composed from island greens and explicitly
  rebound to the current document source.
- Recovery fixes preserve authored whitespace and token ownership for missing
  sequence guards, missing inline-slot `set to`, and missing inline-instance
  `as`.
- All call sites now use `WAML_DEFAULT` or the existing explicit CommonMark
  profile. `CommonMarkCurrent` no longer exists.

## Verification

- Focused Markdown parser/query suites: 21 passed.
- Authority and incremental UML suites: 15 passed.
- Focused Makepad surface: 1 passed.
- Focused OKF concept/citation and authored-index tests: 2 passed.
- Markdown conformance: 6 passed.
- WAML all-features: 517 passed across 31 suites.
- Editor all-features: 908 passed across 10 suites.
- Workspace all-features: 1,655 passed across 61 suites.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`:
  0 errors. Cargo printed only its two known duplicate-package warnings.
- `cargo fmt --all -- --check`: clean after final formatting.

Fuzzing remains deferred to the external fuzz task. This wave ran 0 fuzz
iterations.

RTK global metrics at handoff: 67,457 commands, 237.9M input tokens, 137.0M
output tokens, and 100.9M tokens saved (42.4%).

