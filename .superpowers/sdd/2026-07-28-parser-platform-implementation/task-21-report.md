# Task 21 Report

## Outcome

- Deleted `crates/waml/src/{grammar,parse,syntax,serialize}.rs` and their exports.
- Kept one lossless parser authority under `uml::syntax`; declared layout lowering now consumes
  fixed typed `LayoutPlacement`, `LayoutAlignment`, `LayoutStandalone`, `Operand`,
  `DirectionClause`, `Anchored`, and hint slots directly. No atom flattening or second recursive
  layout grammar remains in `uml::analysis`.
- Restored the historical public `LayoutAtomSyntax` compatibility surface, including both public
  re-exports and `LayoutStatementSyntax::{atoms, typed_atoms}`. These are read-only, ordered,
  lossless token views over the fixed typed tree; production analysis and declared lowering have
  no atom-stream consumer.
- Moved retained semantic layout values to `layout.rs`, relationship-end DTO codecs to `model.rs`,
  and transport bundle splitting to `source.rs`.
- Routed validation, seed verification, UML projection compatibility, formatter, serde/DTO,
  solver, editor, CLI, golden, and benchmark consumers through parser-platform APIs.
- Preserved authored href spelling in syntax/declared values, resolved links relative to the
  referring document, and authored editor scratch placement links with `okf::relative_href`.
- Restored the retired semantic validation matrix with exact severity, path, line, span, and
  revision-scoped range provenance.
- Did not implement or modify Task 22 incremental reparsing.

## Architecture and semantic contracts

- `tests/no_legacy_authority.rs` parses every workspace production Rust target under `src`,
  `examples`, and `build.rs` with `syn`, excluding test-only items. Its qualified module/owner call
  graph resolves imports, renames, associated calls, and typed method receivers; propagates
  lexical, serialization, and reparse capabilities; follows arbitrary type aliases; and rejects
  deleted paths, raw shadow grammar, model-to-source reparsing, and all production callers.
- Eleven adversarial architecture fixtures cover compatibility modules, arbitrary function names,
  `pub(super)`, closures, duplicate names, cross-file and method indirection, imported/qualified
  aliases, split serialization/reparse helpers, qualified allowlists, and legitimate text helpers.
- Layout syntax coverage includes the complete valid matrix, malformed/missing slots, bounded
  recovery, following-row progress, CRLF, UTF-8, exact write-back, exact occurrence ranges, and
  declared valid/incomplete/invalid state.
- Validation covers:
  - standalone and inline `instance of` unresolved/non-classifier warnings;
  - unknown slots, including classifiers with zero attributes;
  - warn-only unresolved instance-authored `links`;
  - warn-only unresolved diagram members;
  - required ends for classifier-to-classifier `associates`, with actor/use-case communication
    links remaining clean;
  - relative layout hrefs, unique and ambiguous bare basenames, unresolved refs, clean placements,
    and directed cycles;
  - exact `Missing` and ambiguous `Order` provenance: literal file/path, line, span, absolute UTF-8
    byte range, document identity/revision, session revision, severity, code, and message across
    CRLF plus a preceding Unicode heading;
  - `LayoutCycle` anchored to the first contradictory placement, never an earlier standalone or
    alignment statement.
- Editor placement previews replace canonical node pairs while authoring nested cross-directory
  targets as relative Markdown hrefs and preserving unrelated authored query/fragment spelling.
- No ignored test or golden contract remains (`#[ignore]` scan is empty).

## RED -> GREEN evidence

- Layout authority RED:
  - `no_legacy_authority`: `1 failed, 1 passed`, listing the analysis-side layout cursor/parser,
    `typed_atoms`, and `LayoutAtomSyntax`.
  - `uml_diagram_syntax`: compile failure because typed `OperandSyntax::value` did not exist.
- Layout authority GREEN:
  - `no_legacy_authority`: `11 passed`, including the complete workspace production scan and
    adversarial AST/call-graph bypass fixtures.
  - `uml_diagram_syntax`: `10 passed`.
- Public compatibility RED:
  - external integration test failed to compile because `LayoutAtomSyntax`, `atoms`, and
    `typed_atoms` were absent.
- Public compatibility GREEN:
  - external integration and behavioral range/order contract: `1 passed`.
- Provenance RED:
  - CRLF standalone layout diagnostics exposed a statement-wide range whose line-local span
    included the newline.
- Provenance GREEN:
  - unresolved diagnostics anchor the typed operand node; exact missing/ambiguous provenance and
    relative/unique clean controls pass.
- Semantic RED:
  - `semantic_diagnostics`: `6 failed, 4 passed`; failures were the zero-attribute slot gap,
    unresolved-member and unresolved-`links` error severities, absent classifier association rule,
    absent inline conformance, and cycle range on a preceding standalone statement.
- Semantic GREEN:
  - `semantic_diagnostics`: `10 passed`.
  - Adjacent classifier/diagram/href/golden suites: `33 passed`.
  - Solver-focused tests: `71 passed`.
- Editor RED:
  - nested candidate regression failed to compile because `placement_candidate` did not exist.
- Editor GREEN:
  - nested candidate regression: `2 passed` across the crate's lib/bin targets.
  - complete editor scene suite: `64 passed`.

## Task 21 commit chain

- `020e5a70` `refactor: retire legacy parser authority`
- `4def39fb` `fix(parser): resolve linked attribute types`
- `0db9c462` `refactor(parser): remove shadow authorities`
- `8b4f9a92` `refactor(uml): require source-backed analysis`
- `3f34252d` `fix(validation): restore UML semantic diagnostics`
- `93bd2da9` `fix(href): preserve authored link spellings`
- `ff2cf682` `test(parser): restore canonical contracts`
- `fe5994e7` `fix(editor): resolve authored layout hrefs`
- `8719f331` `refactor(parser): remove layout reparsing`
- `834c0148` `fix(validation): restore semantic matrix`
- `ec0d0741` `fix(editor): author relative preview hrefs`
- `99dc72ef` `docs(parser): update Task 21 evidence`
- `ca9fa37a` `fix(parser): restore layout atom API`
- `ac163811` `test(parser): enforce syntax authority`
- `81388e1d` `fix(parser): pin layout provenance`
- `014f145` `fix(parser): harden authority graph`

## Verification

- Focused round-three API/architecture/semantic matrix: `32 passed` across 4 suites.
- Complete editor scene suite: `64 passed`.
- `rtk cargo test -p waml --all-features`: `427 passed` across 26 suites.
- `rtk cargo test --workspace --all-features`: `1,274 passed` across 41 suites.
- `rtk cargo check --workspace --all-features`: PASS, 0 errors; four existing warning groups.
- `rtk cargo fmt --all -- --check`: PASS.
- `rtk git diff --check`: PASS.
- `rtk cargo clippy --workspace --all-features`: PASS, 0 errors; 20 existing warnings.
- Strict `rtk cargo clippy --workspace --all-targets --all-features -- -D warnings` stops at the
  pre-existing test-only dead-code error in `crates/waml-syntax/src/red.rs:500`
  (`Code::Error` is never constructed). The file is unchanged by Task 21.
- Independent completion review found no remaining Critical or Important issues after three
  adversarial guard-hardening passes.

## Metrics

- TokenSave code-graph context saved approximately 30,481 tokens in the main thread across the
  three formal review rounds, plus approximately 38,900 tokens reported by the independent
  round-three reviewer.
