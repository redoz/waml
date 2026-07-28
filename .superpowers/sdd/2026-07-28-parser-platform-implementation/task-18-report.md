# Task 18 Report — One-Shot CLI Adapter

## Product changes

- Routed CLI check, show, refs, and list through one invocation-local
  `prepare_candidate(source, None, 0)` analysis snapshot.
- Replaced CLI legacy parse/model/serialize formatting with `uml::Formatter`,
  `ActionContext`, and `SyntaxChangeBatch` lowering.
- Formatting now lowers every claimed document against revision zero, assembles
  one complete candidate, validates it with revision one and the previous
  analyses, and only then exposes output or writes files.
- Generic OKF and index documents remain byte-exact; malformed claimed UML and
  unowned prose retain the established skip behavior.
- Compatibility batches lower against the prepared revision-zero state,
  validate the complete revision-one candidate, and write only after successful
  preparation.
- Physical CLI paths are mapped to validated bundle-relative paths for analysis
  and mapped back through a retained invocation-local root for writes.
- Preserved all `OpDto` tags, versions, and nullable fields, and added a wire
  assertion that invocation revision tokens never serialize.
- Removed all `parse_document`, `build_model`, `serialize_document`, and
  `Line<...>` uses from `waml-cli` and `waml-ops-dto`.

## TDD evidence

- Added real CLI e2e coverage for Generic OKF check, malformed claimed UML
  diagnostics, exact generic no-format output, canonical idempotence, and late
  multi-file failure atomicity before production edits.
- Initial `cargo test -p waml-cli --test cli_e2e` failed in
  `fmt_stdout_preserves_generic_okf_exactly` and
  `fmt_canonical_output_is_idempotent`, proving the legacy formatter path.
- After implementation, the focused suite passes all 11 tests.

## Verification

- `cargo test -p waml-cli --test cli_e2e`: 11 passed.
- `cargo test -p waml-cli`: 54 passed.
- `cargo test -p waml-ops-dto`: 19 passed.
- `cargo test -p waml compat`: 4 passed.
- `cargo test -p waml prepare_candidate`: 2 passed.
- `cargo test -p waml parser_platform_baseline`: 5 passed.
- `cargo test -p waml --test golden`: 6 passed.
- `cargo test -p waml-editor editor_session::tests`: 18 passed.
- Required legacy-authority scan: no matches.

## Workspace baseline note

`cargo test --workspace` and `cargo test --workspace --all-features` reach the
existing suites but fail only in
`crates/waml/tests/serde_shape.rs::package_node_and_model_path` at `okf.rs:394`
(`non-reserved projection produces one concept`). The same failure reproduces
in isolation. Earlier task reports record this as the pre-existing workspace
baseline failure; Task 18 does not modify `okf.rs`, `serde_shape.rs`, or that
compatibility projection.

## TokenSave

TokenSave was used before source inspection and reported approximately 5,044
tokens saved across CLI authority, preparation API, and baseline-failure
queries.
