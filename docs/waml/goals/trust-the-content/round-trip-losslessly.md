# Round-Trip Losslessly

**Goal:** Bytes that the author did not change return without a change.

**Why:** An unrelated rewrite makes a source review inaccurate.

**Done when:** Parse and write keep the exact source bytes, incremental edits
keep unchanged source, and typed edits retain authored source outside their
change.

**Status:** done
**MVP:** yes

## Notes

- `crates/waml-syntax/src/incremental/properties.rs::arbitrary_utf8_is_lossless_and_ranges_navigate`
  checks exact source recovery for generated UTF-8 input.
- `crates/waml-syntax/tests/markdown_inlines.rs::inline_phase_builds_lossless_commonmark_nodes`
  checks exact source recovery for inline CommonMark nodes.
- `crates/waml/src/uml/ops.rs::every_uml_operation_round_trips_authored_source`
  checks authored source after each supported UML operation.
- [Edit Prose](../author-in-the-editor/edit-prose.md) owns text input behavior.
  This goal owns unchanged-source byte accuracy.
