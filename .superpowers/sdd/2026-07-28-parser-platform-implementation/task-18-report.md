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

## Formal fix round 1

### P1 — prepared-snapshot referrer authority

- Added `PreparedCandidate::referrers`, backed only by the immutable
  `uml::Analysis` syntax snapshots and shared catalog produced during
  preparation.
- The query covers attributes, relationships, members, inline instances, and
  linked/bare layout operands without rebuilding a `SourceBundle` or invoking a
  parser.
- CLI `refs` and JSON `show` now consume that same prepared query.
- RED: `cargo test -p waml --test prepared_referrers` failed to compile because
  `PreparedCandidate::referrers` did not exist.
- GREEN: the prepared-referrer integration test passes, and CLI JSON
  show/refs consistency passes.

### P1 — physical display paths

- Added an invocation-local logical `BundlePath` to caller-facing display-path
  map while retaining normalized relative paths for analysis.
- Human/JSON diagnostics and formatter status output render physical paths from
  that map.
- Absolute and relative file spellings, typed directory prefixes, and `stdin`
  remain distinct and current-directory-independent.
- RED: all three focused e2e tests failed: absolute paths collapsed to a
  basename, directory prefixes disappeared, and `stdin` rendered as
  `stdin.md`.
- GREEN: all absolute/relative/directory/stdin display-path tests pass.

### P1 — filesystem transaction atomicity

- Replaced sequential writes/deletes with a complete-set transaction staged
  inside the bundle root.
- Existing targets move into a rollback journal; desired bytes are staged with
  inherited permissions; add/update/delete operations commit by same-volume
  rename; failures unwind in reverse and remove newly created directories.
- `fmt` now submits its complete formatted set to the same transaction used by
  compatibility mutations and emits success output only after commit.
- Added a private filesystem-boundary test seam whose fault implementation
  delegates to real `std::fs::rename` except for one deterministic late call.
- RED: both late-failure tests left the first updated file as `"after"` instead
  of restoring `"before"`.
- GREEN: late write and late delete failures restore every prior byte/path,
  remove all new files/directories/staging artifacts, and return stable errors;
  successful add/update/delete commits as one set and preserves permissions.

### Fix-round verification

- `cargo test -p waml-cli --test cli_e2e`: 15 passed.
- `cargo test -p waml-cli`: 61 passed.
- `cargo test -p waml-ops-dto`: 19 passed.
- `cargo test -p waml --test prepared_referrers`: 1 passed.
- `cargo test -p waml compat`: 4 passed.
- `cargo test -p waml prepare_candidate`: 2 passed.
- `cargo test -p waml parser_platform_baseline`: 5 passed.
- `cargo test -p waml --test golden`: 6 passed.
- `cargo test -p waml --lib`: 440 passed.
- `cargo test -p waml-editor`: 735 passed.
- `cargo check --workspace --all-features`: passed with existing warnings.
- `cargo test --workspace --all-features` still fails only at the documented
  pre-existing `serde_shape::package_node_and_model_path` assertion in
  `okf.rs:394`; all other reported suites pass.

TokenSave reported approximately 10,465 tokens saved during this fix round.
