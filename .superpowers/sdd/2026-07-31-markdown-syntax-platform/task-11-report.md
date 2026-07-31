# Task 11 report

- Added a malformed-source recovery matrix. It covers BOM, CRLF, tabs, Unicode,
  combining characters, mixed endings, fences, inline forms, tables, HTML,
  frontmatter, and WAML headings in protected containers.
- The strengthened matrix found no legitimate recovery gap. The first RED was an
  incorrect raw-HTML expectation; filtered HTML already has a typed diagnostic.
- Added a randomized one-to-eight edit oracle. It found a WAML section wrapper
  mismatch after a context-changing edit. Incremental parsing now verifies the
  WAML island count against a clean parse and uses `IslandBoundaryChanged` when
  it differs.
- Updated both local fuzz harnesses and added four requested seeds.

Verification:

- `markdown_recovery`: 1 passed.
- `properties`: 3 passed, including 512 generated edit sequences.
- `markdown_incremental`: 12 passed.
- `cargo check --manifest-path fuzz/Cargo.toml --bins`: 0 errors.
- The requested default 10,000-run fuzz commands build both targets, but Windows
  cannot load their sanitizer runtime (`STATUS_DLL_NOT_FOUND`) before iteration
  1. A `--sanitizer none` retry cannot link because MSVC has no `sancov` section
  symbols. Therefore both required run counts are 0; no target assertion ran.

RTK global savings at completion: 100.1M tokens (42.3%).

Review fixes:

- Expanded the incremental oracle to compare all public Markdown query families
  and typed metadata, normalized reference backlinks and resolution, diagnostics,
  recovery spans, and WAML island descriptors. Identity values are excluded.
- Preserved two generated failures as deterministic tests. A reference paste into
  a heading now uses `UnsafeSynchronization` because a local window cannot resolve
  definitions outside that window. A width-changing edit before a reference
  definition also uses `UnsafeSynchronization` because it can shift absolute
  destination ranges in reused annotations. Same-width edits remain incremental.
- Enumerated recovery syntax nodes, bad tokens, missing tokens other than the EOF
  sentinel, and semantic recovery spans. Every enumerated range must satisfy
  `has_recovery`.
- Extended `parse_write` to call every public Markdown query family and validate
  all returned ranges. Both fuzz targets compile with zero errors.
- Removed generated `properties.proptest-regressions` files after converting each
  seed into a named deterministic regression.

Review verification:

- Focused reference-use regression: 1 passed.
- Focused destination-range regression: 1 passed.
- `markdown_recovery`: 1 passed.
- `properties`: 3 passed with 512 generated edit sequences.
- `markdown_incremental`: 12 passed.
- `cargo check --manifest-path fuzz/Cargo.toml --bins`: 0 errors, 4 existing
  warnings.
- TokenSave saved approximately 38.8k source tokens across Task 11 implementation
  and review.
