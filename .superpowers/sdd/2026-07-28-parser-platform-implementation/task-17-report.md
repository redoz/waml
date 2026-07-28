# Task 17 report

Implementation commit: `402582b`

## Outcome

- Added `EditorSession::snapshot`, a single borrowed snapshot containing current
  and persisted source, OKF/UML analyses, revision, and dirty provenance.
- Changed `ViewData`, native document providers, navigation, tree decoration,
  view synchronization, format/repair preparation, and native save to consume
  that revision-coherent analysis snapshot.
- Kept provider composition explicitly UML-first then Generic OKF fallback.
  Invalid claimed UML remains UML-owned; arbitrary, missing, and unknown UML
  types fall back to Generic OKF; Index and Log remain non-openable.
- Added revision-bound `PreparedAction` adapters for UML formatting and
  missing-colon/type/invalid-multiplicity repair actions. Each action becomes a
  `SyntaxChangeBatch` wrapped by `PendingEdit`.
- Added declared-state inspector rendering so invalid-present authored
  attributes remain visible with recovery labels instead of disappearing from
  the validated projection.
- Replaced provider-prepared live views after a session revision even when the
  provider/tab identity is unchanged, while retaining preview/persistent tab
  state.
- Bound native save to one dirty snapshot and reject clean/stale snapshots
  before touching disk. Existing conflict feedback remains user-visible through
  the status bar and retry path.
- Removed editor-source references to `parse_document`, `build_model`,
  `serialize_document`, legacy `Line<T>`, and direct `uml::project`.

TokenSave was used before source inspection. Reported savings were
approximately 8,554 tokens across the planning/context queries.

## Verification

Passing:

- `rtk cargo test -p waml-editor editor_session::tests` — 18 passed
- `rtk cargo test -p waml-editor documents::tests` — 7 passed
- `rtk cargo test -p waml-editor uml_documents::tests` — 1 passed
- `rtk cargo test -p waml-editor okf_documents::tests` — 2 passed
- `rtk cargo test -p waml-editor document_host::tests` — 7 passed
- `rtk cargo test -p waml-editor generic_okf_view::tests` — 4 passed
- `rtk cargo test -p waml-editor nav::tests` — 13 passed
- `rtk cargo test -p waml-editor native_save::tests` — 14 passed
- `rtk cargo test -p waml-editor inspector::parser_recovery_tests` — passed
  in both native targets
- `rtk cargo check -p waml-editor` — passed
- `rtk cargo test -p waml-cli --test lsp_e2e` — 3 passed
- `rtk cargo fmt --all -- --check` — passed
- prohibited editor scan — no matches
- `DocumentHost` production scan — no provider/family dispatch

Native recovery evidence is covered by automated tests for invalid-present
inspector rows, repair action titles/provenance, stale/clean save rejection,
save-conflict non-overwrite behavior, and provider-prepared view replacement.
The Task 17 brief does not require a screenshot, so no native window capture was
added.

## Broader-gate concerns

- `rtk cargo test -p waml-editor` reaches 129 passing tests and has four failures
  in legacy UI fixture assertions:
  `inspector::tests::diagram_view_projects_identity_only` and three scene
  placement/conflict tests. Those fixtures previously reached the removed
  legacy test-only projection helper. The shared UML analysis currently emits
  an empty diagram profile and does not validate the fixture's linked layout
  statements into placement relations. The Task 17 cutover intentionally does
  not recreate an editor-local projection side path.
- `rtk cargo test --workspace` stops in the separate `waml` example target at
  `package_node_and_model_path` (`okf.rs:394`, “non-reserved projection produces
  one concept”) after its preceding suites pass. This is outside Task 17's
  native editor ownership.
- `cargo check` reports only existing duplicate Makepad dependency notices and
  dead-code warnings for compatibility accessors `EditorSession::source` and
  `persisted_bundle`.

## Formal fix round 1

Fix commit: `140e39d`

The review found two real cutover regressions. Systematic root-cause tracing and
strict TDD were used for both; TokenSave reported approximately 6,745 tokens
saved during the initial fix-round exploration.

### Red evidence

- `diagram_projection_preserves_profile` failed with `left: ""`,
  `right: "uml-domain"`. The shared analyzer explicitly installed an empty
  profile instead of lowering canonical OKF frontmatter.
- `diagram_projection_preserves_complete_two_link_placement` failed because the
  projected diagram had no validated placement and its declared layout field
  was `Invalid`. `parse_layout_anchored` returned `None` before parsing when the
  first operand was a typed link token.
- `contradictory_linked_placements_reach_shared_solver_conflict_diagnostics`
  failed because no linked placements reached the solver, so no placement was
  dropped and no `LayoutConflict` diagnostic was produced.
- The real native picker test
  `picker_selection_keeps_declared_recovery_rows_and_revision_bound_actions`
  selected the invalid classifier but failed with zero inspector attribute rows
  instead of one. The picker apply path rebuilt the subject from the validated
  projection via legacy `set_subject`.

### Fixes

- The shared UML analyzer now lowers diagram `profile` from the OKF Concept's
  canonical extra frontmatter.
- Layout declaration parsing no longer treats a leading link operand as an
  absent anchored operand. Complete linked placements retain both operands and
  directions, and contradictory linked relations reach shared solver conflict
  diagnostics.
- Picker close and apply now carry the exact `&uml::Analysis` snapshot and call
  `set_subject_analysis`, preserving declared invalid-present fields.
- The picker integration test also proves missing-colon and invalid-multiplicity
  repairs remain available from that snapshot and that a second action becomes
  stale after the first commits.

### Green evidence

- `rtk cargo test -p waml --test uml_diagram_syntax` — 9 passed
- All four previously failing editor parity fixtures — passed
- Picker recovery integration test — passed
- `rtk cargo test -p waml-editor editor_session::tests` — 18 passed
- `rtk cargo test -p waml-editor document_host::tests` — 7 passed
- `rtk cargo test -p waml-editor nav::tests` — 13 passed
- `rtk cargo test -p waml` — 552 passed across 21 suites
- `rtk cargo test -p waml-editor --all-features` — 735 passed across 5 suites
- `rtk cargo check --workspace --all-targets` — passed
- `rtk cargo fmt --all -- --check` — passed
- `rtk git diff --check` — passed
- Prohibited editor legacy-authority scan — no matches

The workspace check retains only pre-existing dead-code notices in test/support
code and duplicate Makepad dependency notices. No new warning remains from the
fix.
