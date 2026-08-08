# Changed files

- `docs/waml/documentation-contract.md`
- `docs/waml/waml-feature-gaps.md`
- `docs/superpowers/audits/reports/contract.md`

# Scenario identifiers

None.

# Evidence

`rtk cargo run -p waml-cli -- check docs/waml/documentation-contract.md docs/waml/waml-feature-gaps.md` exits 0 and reports `No problems found.`

`rtk node scripts/check-waml-doc-contract.mjs docs/waml` reports no defect in
`documentation-contract.md` or `waml-feature-gaps.md`. It has the following
known pre-migration findings outside these files:

- `docs/waml/architecture/views/authoring-and-validation.md:1`,
  `domain-model.md:1`, `editing-round-trip.md:1`, `index.md:1`,
  `layout-solving.md:1`, `model-vocabulary.md:1`, `share-round-trip.md:1`,
  `system-context.md:1`, and `web-delivery.md:1`: `view needs frontmatter sources or a link to ../concepts/implementation/`.
- Invalid `**Status:**` values in the Task 7 documents under
  `docs/waml/goals/author-in-the-editor/`: `arrange-a-diagram.md:14`,
  `author-with-the-keyboard.md:13`, `create-and-delete-documents.md:13`,
  `draw-on-the-canvas.md:16`, `edit-prose.md:14`, `edit-the-model.md:15`,
  `reduce-the-effort.md:26`, `save-and-undo.md:13`, and
  `start-from-a-template.md:17`.
- Invalid `**Status:**` values in the Task 6 documents under
  `docs/waml/goals/read-a-bundle/`: `browse-the-tree.md:13`,
  `fit-the-window.md:16`, `navigate-and-return.md:12`, `open-a-bundle.md:12`,
  `read-a-diagram.md:12`, and `read-a-document.md:13`.
- Invalid `**Status:**` values in the Task 10 documents under
  `docs/waml/goals/share-and-publish/`: `export-a-bundle.md:15`,
  `publish-a-site.md:12`, `run-in-a-browser.md:13`, `serve-locally.md:16`, and
  `share-a-link.md:13`; under `docs/waml/goals/tooling-around-the-repo/`:
  `command-line-tool.md:13`, `language-server.md:14`, and
  `text-editor-integration.md:13`.
- Invalid `**Status:**` values in the Task 7 documents under
  `docs/waml/goals/trust-the-content/`: `format-canonically.md:13`,
  `keep-indexes-correct.md:12`, `report-every-problem.md:13`,
  `resolve-references.md:13`, and `round-trip-losslessly.md:13`.
- Invalid `**Status:**` values in the Task 8 and 9 documents:
  `docs/waml/goals/uml/activity/feature-cut.md:9`,
  `docs/waml/goals/uml/class/feature-cut.md:9`,
  `docs/waml/goals/uml/sequence/feature-cut.md:9`,
  `docs/waml/goals/uml/shared/keep-the-map-stable.md:15`,
  `docs/waml/goals/uml/shared/place-the-labels.md:14`,
  `docs/waml/goals/uml/shared/route-the-edges.md:13`,
  `docs/waml/goals/uml/shared/select-and-inspect.md:14`,
  `docs/waml/goals/uml/shared/solve-the-layout.md:13`,
  `docs/waml/goals/uml/shared/theme-the-diagram.md:13`,
  `docs/waml/goals/uml/state-machine/feature-cut.md:9`, and
  `docs/waml/goals/uml/use-case/feature-cut.md:9`.
- `docs/waml/goals/uml/sequence/language.md` needs its planned scenario
  migration. At heading lines 94, 103, 111, 119, 126, 132, 140, 147, 155, 162,
  171, 178, 185, 192, 198, 205, 211, 217, 224, 230, 237, 246, 253, 259, 265,
  271, 278, 286, 295, 305, and 315, the checker reports `evidence has no
  source or valid marked test`, `expected an **Evidence:** line`, and `native
  test is absent; add an item under Verification gaps`. At lines 96, 105, 113,
  121, 128, 134, 142, 149, 157, 164, 173, 180, 187, 194, 200, 207, 213, 219,
  226, 232, 239, 248, 255, 261, 267, 273, 280, 288, 297, 307, and 317, it
  reports `expected **Applies to:** shared, native, or browser`.

# Verification gaps

None in the Task 5 documents. The sequence-language findings above are owned
by the behavior-diagram goal stream.

# Open discrepancies

None.

# Feature gaps

- `FG-001` through `FG-010` are recorded in `docs/waml/waml-feature-gaps.md`.
- `FG-010` records scenario-to-product-use-case-to-test-and-evidence
  traceability. It does not record specialized actor, use-case, or
  system-boundary rendering.
