# Task 5 report

Implemented revision-scoped OKF analysis with stable document identities,
revision updates, shell snapshots, structure maps, and an OKF shell entry
point. Added focused catalog coverage for unchanged, changed, added, removed,
and renamed documents.

Verification:

- `rtk cargo test -p waml --test analysis_catalog` — 2 passed
- `rtk cargo test -p waml analysis::tests::candidate_failure_is_non_mutating` — 1 passed
- `rtk cargo test -p waml okf::tests` — 14 passed
- `rtk cargo test -p waml --test serde_shape` — 0 passed (feature-gated suite)
- `rtk cargo test -p waml` — 444 passed
- `rtk cargo fmt --check`
- `rtk git diff --check`

## Fix round 1

- Moved OKF projection decisions into `okf/shell.rs`; derivation now consumes
  the candidate catalog, syntax snapshots, and Markdown structure maps.
- Added exact candidate source/tree, provenance, structure-bound, and catalog
  width validation with shell-stage structural invariant errors.
- Expanded private hook tests across Shell and OKF phases, including phase
  counts, committed-analysis identity, and failed ID/revision allocation reuse.
- Mapped parser structural invariants to `AnalysisStage::Shell` and preserved
  malformed unclosed-frontmatter recovery without promoting recovered metadata.

Verification:

- `rtk cargo test -p waml analysis::tests` — 4 passed
- `rtk cargo test -p waml --test analysis_catalog` — 2 passed
- `rtk cargo test -p waml okf::tests` — 14 passed
- `rtk cargo test -p waml` — 447 passed
- `rtk cargo check --workspace` — 0 errors (2 existing duplicate-package warnings)
- `rtk cargo fmt --check`
- `rtk git diff --check`
