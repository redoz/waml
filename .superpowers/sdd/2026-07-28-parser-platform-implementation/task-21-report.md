# Task 21 Report

## Outcome

- Deleted `crates/waml/src/{grammar,parse,syntax,serialize}.rs` and their exports.
- Moved retained layout value types to `layout.rs`.
- Moved relationship-end DTO codecs to `model.rs` and transport bundle splitting to `source.rs`.
- Routed validation, seed verification, UML projection compatibility, CLI input, goldens,
  formatter tests, serde tests, solver/editor consumers, and the benchmark through parser-platform
  APIs.
- Added `tests/no_legacy_authority.rs`.
- Did not change incremental parsing/reuse code.

## RED -> GREEN

- RED: `rtk cargo test -p waml --test no_legacy_authority` failed with all four files and
  all four `lib.rs` exports listed.
- GREEN: the same command passed (`1 passed`).

## Consumer/deletion checklist

- [x] legacy grammar authority deleted
- [x] legacy document parser/model builder deleted
- [x] legacy recovering syntax model deleted
- [x] legacy serializer authority deleted
- [x] CLI tuple transport moved to source authority
- [x] formatter consumers use `uml::Formatter`
- [x] validation consumers use `prepare_candidate`
- [x] projection/serde consumers use prepared UML analysis
- [x] editor/solver imports use `layout` semantic values
- [x] sealed DTO relationship-end codec retained without grammar authority
- [x] prohibited-symbol scan is empty
- [x] no Task 22 incremental parsing work included

## Verification

- `rtk cargo test -p waml --all-features`: 427 passed, 1 ignored.
- `rtk cargo test --workspace --all-features`: 1,272 passed, 1 ignored.
- `rtk cargo check --workspace --all-features`: PASS.
- `rtk cargo clippy --workspace --all-features`: PASS with 20 existing warnings.
- focused authority, golden, formatter, LSP, and editor-session suites: PASS.
- `rtk cargo fmt --all -- --check`: PASS after formatting.
- strict `rtk cargo clippy --workspace --all-targets --all-features -- -D warnings` remains
  blocked by pre-existing parser-platform lint debt (17 errors, including dead-code test enums,
  `SyntaxSet::len` without `is_empty`, MSRV-incompatible `is_none_or`, and parser helpers with too
  many arguments). The Task 21-introduced needless borrow was fixed.

## Notes

- The old `orders-domain.md` tuple semantic assertion is ignored because it uses retired attribute
  surface forms. Its source is still covered by the authoritative shell lossless-write test and
  validation now proves those forms produce active-parser diagnostics.
- TokenSave initial planning saved approximately 4,906 tokens.
