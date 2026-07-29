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
- Hardened the residual `syn` authority guard through successive adversarial reviews: qualified
  trust roots, binding identities and lexical scopes, destructured and assigned aliases,
  cached/wrapped value evidence, evaluation order, branch/control joins, and loop-carried state
  now fail closed when protected model text can reach a parser-like dispatch.
- Kept the production missing-target edit error as the static literal `"no claimed concept"` in
  `uml::lower`; the guard work did not introduce a second parser or model-to-source authority.
- Did not implement or modify Task 22 incremental reparsing.
- Preserved the unrelated dirty Task 7 report without staging or modifying it.

## Architecture and semantic contracts

- `tests/no_legacy_authority.rs` obtains actual workspace members and production targets from
  `cargo metadata`. It follows custom lib/bin/example/build target paths, external `mod` files,
  inline modules, `#[path]`, and literal `include!` sources instead of assuming `crates/*/src`.
  A committed nested fixture includes an out-of-directory workspace member and proves all of those
  source shapes are audited.
- Package dependency checks enforce `waml-syntax <- waml <- retained hosts`, with DTO composition
  explicit and direct host-to-syntax dependencies rejected. The raw UML parser module is visible
  only inside `crate::uml`; an external Cargo compile-fail fixture proves it is not a public API.
- The 28-test authority suite covers compatibility modules, arbitrary function names,
  private/`pub(super)` entries, closures and async blocks, function pointers/callable locals,
  duplicate names, cross-file, chained/field receiver and trait dispatch, imported/qualified and
  destructured aliases, assignment and shadowing, direct grammar construction, split
  serialization/reparse helpers, qualified syntax reparsing, macro/include/generated policy,
  qualified allowlists, visible model-to-source surfaces, evaluation order, branch and loop
  control flow, and legitimate label/render/analyze/trim/to-string helpers.
- The harmless standard-collection exemption is identity-based: only a proven prelude `Vec`,
  `std::vec::Vec`, or `alloc::vec::Vec` receiver is trusted. User-defined `Vec` types and
  unresolved or ambiguous `Vec` imports remain conservative reparse sinks.
- The AST pass is residual rather than a substitute for rustc. It checks raw-to-protected-grammar
  signatures, exact qualified type aliases, visible model-to-text surfaces, and conservative
  model-to-authority-reparse reachability. It does not claim general type inference or external
  macro expansion. Literal Rust includes are followed; dynamic/generated includes fail closed;
  local opaque macros that mention protected grammar types fail closed.
- Residual control flow is explicit: `Fallthrough`, `Return`, `Break`, `Continue`, and `Diverge`
  outcomes sequence value operands before their terminal effect, branches join only reachable
  environments, and value loops compute a monotone fixed point over entry, body-fallthrough, and
  continue environments before deriving break values and exit state.
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
- Architecture round-four RED:
  - metadata fixture failed at the old hard-coded `crates` directory scan;
  - raw parser external compile fixture compiled successfully, proving the module was public;
  - direct construction, callable-local, field/trait receiver, qualified reparse, macro, and
    visible model-to-source fixtures all escaped the old partial resolver.
- Architecture round-four GREEN:
  - `no_legacy_authority`: `17 passed`, including Cargo-aware target/module discovery, dependency
    direction, compile-fail visibility, every reviewer bypass, and legitimate-name controls;
  - full Task 21 API/architecture/semantic matrix: `38 passed` across 4 suites.
- Formal guard-review RED rounds exposed qualified-identity spoofing, incomplete alias/cache/type
  evidence, destructuring and assignment gaps, initializer/scope leakage, ambiguous `Vec` trust,
  deferred closure and async execution, argument/composite evaluation order, unreachable branch
  joins, and incomplete break/continue/return modeling.
- The final two RED probes independently demonstrated:
  - a `continue` path tainting `rendered` on one iteration before `break rendered` on the next;
  - a `break 'skip` nested inside a `return` operand, where the operand exits before the return and
    keeps the tainted branch reachable.
- Formal guard-review GREEN:
  - both exact probes now reach the conservative unresolved-dispatch finding;
  - `no_legacy_authority`: `28 passed`;
  - independent staff review of `f2d60e8` against `1f375e04` returned **READY**, with no remaining
    blocking finding.

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
- `e797e86` `fix(parser)!: seal raw authority boundary`
- `a9f024ad` `docs(parser): record authority redesign`
- `1f375e04` `docs(parser): correct lint evidence`
- `0dfde6b6` `fix(parser): close authority guard bypasses`
- `69d86f52` `fix(parser): close residual guard escapes`
- `e52959d3` `fix(parser): reject guard identity spoofing`
- `5f2cc7a6` `fix(parser): qualify guard trust roots`
- `eb83ae09` `fix(parser): prove guard trust boundaries`
- `a464133a` `fix(parser): prove alias and cache shapes`
- `7d657582` `fix(parser): close control-flow guard gaps`
- `1c836b51` `fix(parser): retain guard type modules`
- `81d2cd16` `chore(parser): keep guard lint-clean`
- `78337cc7` `fix(parser): track destructured guard aliases`
- `c94f303b` `fix(parser): preserve wrapped type evidence`
- `2b1b7294` `fix(parser): update assigned alias types`
- `d3e3cf60` `fix(parser): scope guard binding identities`
- `65bd2989` `fix(parser): respect let initializer scope`
- `022ad97d` `fix(parser): derive aliases after initialization`
- `7a5f004a` `fix(parser): close scoped taint escapes`
- `85d6118c` `fix(parser): reject ambiguous Vec imports`
- `c10cd870` `fix(parser): evaluate cached call taint`
- `e50a55a0` `fix(parser): retain value-wrapper taint`
- `6a4ac4c2` `fix(parser): preserve taint execution order`
- `4fe1c81a` `fix(parser): join guard execution paths`
- `ed3379b2` `fix(parser): model guard control exits`
- `f2d60e88` `fix(parser): converge loop taint flow`

## Verification

- Final Task 21 API/architecture/semantic matrix: `49 passed` across 4 suites.
- Final authority suite: `28 passed`, including raw-authority compile-fail,
  Cargo/module/dependency, adversarial call-edge, control-flow, and legitimate-control fixtures.
- Dedicated Cargo metadata custom-target and outside-workspace-member filter: `3 passed`.
- Complete `waml-editor` package gate: `737 passed` across 5 target-expanded suites.
- `rtk cargo test -p waml --all-features`: `444 passed` across 26 suites.
- `rtk cargo test --workspace --all-features`: `1,291 passed` across 41 suites.
- `rtk cargo check --workspace --all-features`: PASS, 0 errors; four existing warning groups.
- `rtk cargo fmt --all -- --check`: PASS.
- `rtk git diff --check`: PASS.
- `rtk cargo clippy --workspace --all-features`: PASS, 0 errors; 20 existing warnings.
- `rtk cargo clippy --workspace --all-targets --all-features`: PASS, 0 errors; 29 existing
  warnings.
- Independent completion review found both final control-flow blockers resolved and returned
  **READY**.

## Metrics

- TokenSave code-graph context saved approximately 215,000 tokens in the main implementation
  thread across the architecture redesign and formal fix rounds.
- Independent review rounds reported approximately 67,000 additional tokens saved.
