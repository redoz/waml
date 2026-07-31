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
- `properties`: 1 passed.
- `markdown_incremental`: 12 passed.
- `cargo check --manifest-path fuzz/Cargo.toml --bins`: 0 errors.
- The requested default 10,000-run fuzz commands build both targets, but Windows
  cannot load their sanitizer runtime (`STATUS_DLL_NOT_FOUND`) before iteration
  1. A `--sanitizer none` retry cannot link because MSVC has no `sancov` section
  symbols. Therefore both required run counts are 0; no target assertion ran.

RTK global savings at completion: 100.1M tokens (42.3%).
