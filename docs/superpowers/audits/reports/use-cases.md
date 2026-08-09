# Changed files

- `docs/waml/use-cases/index.md`
- `docs/waml/use-cases/actors/index.md`
- `docs/waml/use-cases/actors/author.md`
- `docs/waml/use-cases/actors/reader.md`
- `docs/waml/use-cases/actors/command-line-user.md`
- `docs/waml/use-cases/actors/automation-client.md`
- `docs/waml/use-cases/actors/language-client.md`
- `docs/waml/use-cases/workflows/index.md`
- 25 shipped workflow leaves under `docs/waml/use-cases/workflows/`.
- `docs/waml/use-cases/views/index.md`
- `docs/waml/use-cases/views/editor-workflows.md`
- `docs/waml/use-cases/views/browser-and-publishing-workflows.md`
- `docs/waml/use-cases/views/tooling-workflows.md`
- `docs/waml/waml-feature-gaps.md`
- `docs/superpowers/audits/reports/use-cases.md`

# Scenario identifiers

- `edit-prose` — `NATIVE-023` through `NATIVE-026`, `NATIVE-046`, and `NATIVE-047`.
- `save-and-undo` — `NATIVE-027` through `NATIVE-031`, `NATIVE-048`, `NATIVE-049`, `NATIVE-056`, `NATIVE-057`, and `NATIVE-062`.
- `browse-the-tree` — `NATIVE-010` through `NATIVE-012`, and `NATIVE-015`.
- `fit-the-window` — `NATIVE-006` and `NATIVE-007`.
- `navigate-and-return` — `NATIVE-013` and `NATIVE-016`.
- `open-a-bundle` — `NATIVE-001` through `NATIVE-005`, and `NATIVE-014`.
- `read-a-document` — `NATIVE-022`, `NATIVE-045`, and `NATIVE-061`.
- `use-the-shell` — `NATIVE-008`, `NATIVE-009`, and `NATIVE-055`.
- `work-with-tabs` — `NATIVE-017` through `NATIVE-020`.
- `export-a-bundle` — `BROWSER-001`, `BROWSER-003`, `BROWSER-004`, `BROWSER-006`, `BROWSER-011`, and `BROWSER-016`.
- `publish-a-site` — `BROWSER-012`.
- `run-in-a-browser` — `BROWSER-005`.
- `serve-locally` — `BROWSER-002`, `BROWSER-007` through `BROWSER-010`, and `BROWSER-017` through `BROWSER-020`.
- `share-a-link` — `CLI-003` and `BROWSER-015`.
- `command-line-tool` — `CLI-001`, `CLI-002`, and `CLI-004` through `CLI-011`.
- `language-server` — `LSP-001` through `LSP-005`.
- `text-editor-integration` — `VSCODE-001` through `VSCODE-006`, and `VSCODE-008`.
- `report-every-problem` — `NATIVE-032` and `NATIVE-050`.
- `interact-with-an-activity-diagram` — `NATIVE-037` through `NATIVE-039`, `NATIVE-053`, `NATIVE-063`, and `NATIVE-064`.
- `interact-with-a-class-diagram` — `NATIVE-033`, `NATIVE-035`, `NATIVE-058` through `NATIVE-060`, `NATIVE-065` through `NATIVE-067`, `NATIVE-069`, and `NATIVE-072`.
- `interact-with-a-sequence-diagram` — `NATIVE-040` through `NATIVE-044`, and `NATIVE-054`.
- `sequence-language` — `SEQ-MSG-1` through `SEQ-MSG-8`, `SEQ-ORD-1`, `SEQ-ORD-2`, `SEQ-FRAG-1` through `SEQ-FRAG-11`, `SEQ-BND-1` through `SEQ-BND-6`, `SEQ-ACT-1`, `SEQ-ACT-2`, `SEQ-EXC-1`, `SEQ-REC-1`, and `SEQ-REC-2`.
- `route-the-edges` — `NATIVE-051`.
- `select-and-inspect` — `NATIVE-034`, `NATIVE-052`, `NATIVE-068`, `NATIVE-070`, and `NATIVE-071`.
- `solve-the-layout` — `NATIVE-036`.

# Evidence

The semantic-intention key is the deterministic workflow leaf slug. The 25
intention groups contain 144 shipped scenario headings. Of these headings, 112
come from the frozen inventory and 32 come from the shipped sequence-language
contract that Task 9 kept stable.

## Actor membership and workflow mapping

| Actor leaf | External role | Associated workflows | Views |
| --- | --- | --- | --- |
| `author.md` | Person who changes WAML content. | `edit-prose`, `fit-the-window`, `interact-with-a-class-diagram`, `report-every-problem`, `save-and-undo`, `select-and-inspect`, `sequence-language`, `text-editor-integration` | `editor-workflows`, `tooling-workflows` |
| `reader.md` | Person who opens and reads WAML content. | `browse-the-tree`, `export-a-bundle`, `fit-the-window`, `interact-with-an-activity-diagram`, `interact-with-a-class-diagram`, `interact-with-a-sequence-diagram`, `navigate-and-return`, `open-a-bundle`, `read-a-document`, `route-the-edges`, `run-in-a-browser`, `select-and-inspect`, `serve-locally`, `share-a-link`, `solve-the-layout`, `use-the-shell`, `work-with-tabs` | `editor-workflows`, `browser-and-publishing-workflows` |
| `command-line-user.md` | Person who starts WAML commands from a shell. | `command-line-tool`, `export-a-bundle`, `serve-locally`, `share-a-link` | `browser-and-publishing-workflows`, `tooling-workflows` |
| `automation-client.md` | External process that starts WAML commands or publication work. | `command-line-tool`, `publish-a-site` | `browser-and-publishing-workflows`, `tooling-workflows` |
| `language-client.md` | External text editor that requests WAML language features. | `language-server` | `tooling-workflows` |

## Use-case ownership and view membership

| Use-case leaf | Semantic intention | Owning goal | Scenarios | Actors | Views |
| --- | --- | --- | --- | --- | --- |
| `browse-the-tree.md` | `browse-the-tree` | `goals/read-a-bundle/browse-the-tree.md` | `NATIVE-010..012`, `NATIVE-015` | Reader | `editor-workflows` |
| `command-line-tool.md` | `command-line-tool` | `goals/tooling-around-the-repo/command-line-tool.md` | `CLI-001`, `CLI-002`, `CLI-004..011` | Automation Client; Command-Line User | `tooling-workflows` |
| `edit-prose.md` | `edit-prose` | `goals/author-in-the-editor/edit-prose.md` | `NATIVE-023..026`, `NATIVE-046`, `NATIVE-047` | Author | `editor-workflows` |
| `export-a-bundle.md` | `export-a-bundle` | `goals/share-and-publish/export-a-bundle.md` | `BROWSER-001`, `BROWSER-003`, `BROWSER-004`, `BROWSER-006`, `BROWSER-011`, `BROWSER-016` | Command-Line User; Reader | `browser-and-publishing-workflows`; `tooling-workflows` |
| `fit-the-window.md` | `fit-the-window` | `goals/read-a-bundle/fit-the-window.md` | `NATIVE-006`, `NATIVE-007` | Author; Reader | `editor-workflows` |
| `interact-with-an-activity-diagram.md` | `interact-with-an-activity-diagram` | `goals/uml/activity/interact-with-an-activity-diagram.md` | `NATIVE-037..039`, `NATIVE-053`, `NATIVE-063`, `NATIVE-064` | Reader | `editor-workflows` |
| `interact-with-a-class-diagram.md` | `interact-with-a-class-diagram` | `goals/uml/class/interact-with-a-class-diagram.md` | `NATIVE-033`, `NATIVE-035`, `NATIVE-058..060`, `NATIVE-065..067`, `NATIVE-069`, `NATIVE-072` | Author; Reader | `editor-workflows` |
| `interact-with-a-sequence-diagram.md` | `interact-with-a-sequence-diagram` | `goals/uml/sequence/interact-with-a-sequence-diagram.md` | `NATIVE-040..044`, `NATIVE-054` | Reader | `editor-workflows` |
| `language-server.md` | `language-server` | `goals/tooling-around-the-repo/language-server.md` | `LSP-001..005` | Language Client | `tooling-workflows` |
| `navigate-and-return.md` | `navigate-and-return` | `goals/read-a-bundle/navigate-and-return.md` | `NATIVE-013`, `NATIVE-016` | Reader | `editor-workflows` |
| `open-a-bundle.md` | `open-a-bundle` | `goals/read-a-bundle/open-a-bundle.md` | `NATIVE-001..005`, `NATIVE-014` | Reader | `editor-workflows` |
| `publish-a-site.md` | `publish-a-site` | `goals/share-and-publish/publish-a-site.md` | `BROWSER-012` | Automation Client | `browser-and-publishing-workflows` |
| `read-a-document.md` | `read-a-document` | `goals/read-a-bundle/read-a-document.md` | `NATIVE-022`, `NATIVE-045`, `NATIVE-061` | Reader | `editor-workflows` |
| `report-every-problem.md` | `report-every-problem` | `goals/trust-the-content/report-every-problem.md` | `NATIVE-032`, `NATIVE-050` | Author | `editor-workflows` |
| `route-the-edges.md` | `route-the-edges` | `goals/uml/shared/route-the-edges.md` | `NATIVE-051` | Reader | `editor-workflows` |
| `run-in-a-browser.md` | `run-in-a-browser` | `goals/share-and-publish/run-in-a-browser.md` | `BROWSER-005` | Reader | `browser-and-publishing-workflows` |
| `save-and-undo.md` | `save-and-undo` | `goals/author-in-the-editor/save-and-undo.md` | `NATIVE-027..031`, `NATIVE-048`, `NATIVE-049`, `NATIVE-056`, `NATIVE-057`, `NATIVE-062` | Author | `editor-workflows` |
| `select-and-inspect.md` | `select-and-inspect` | `goals/uml/shared/select-and-inspect.md` | `NATIVE-034`, `NATIVE-052`, `NATIVE-068`, `NATIVE-070`, `NATIVE-071` | Author; Reader | `editor-workflows` |
| `sequence-language.md` | `sequence-language` | `goals/uml/sequence/language.md` | 32 stable `SEQ-*` scenarios | Author | `editor-workflows`; `tooling-workflows` |
| `serve-locally.md` | `serve-locally` | `goals/share-and-publish/serve-locally.md` | `BROWSER-002`, `BROWSER-007..010`, `BROWSER-017..020` | Command-Line User; Reader | `browser-and-publishing-workflows`; `tooling-workflows` |
| `share-a-link.md` | `share-a-link` | `goals/share-and-publish/share-a-link.md` | `CLI-003`, `BROWSER-015` | Command-Line User; Reader | `browser-and-publishing-workflows`; `tooling-workflows` |
| `solve-the-layout.md` | `solve-the-layout` | `goals/uml/shared/solve-the-layout.md` | `NATIVE-036` | Reader | `editor-workflows` |
| `text-editor-integration.md` | `text-editor-integration` | `goals/tooling-around-the-repo/text-editor-integration.md` | `VSCODE-001..006`, `VSCODE-008` | Author | `tooling-workflows` |
| `use-the-shell.md` | `use-the-shell` | `goals/read-a-bundle/use-the-shell.md` | `NATIVE-008`, `NATIVE-009`, `NATIVE-055` | Reader | `editor-workflows` |
| `work-with-tabs.md` | `work-with-tabs` | `goals/read-a-bundle/work-with-tabs.md` | `NATIVE-017..020` | Reader | `editor-workflows` |

The actor leaf set equals the union of the three `### External actors` groups.
The use-case leaf set equals the union of the three named product-boundary
groups. Cross-boundary workflows occur in each applicable view.

## Frozen owner reconciliation for Task 12

The completed goal leaves reconcile the workflow owners, but 35 frozen
`goal_document` values still name the pre-integration location. Task 12 must
correct these inventory values. Task 11 does not invent a second owner.

- `NATIVE-037..039`, `NATIVE-053`, `NATIVE-063`, and `NATIVE-064` now occur in `goals/uml/activity/interact-with-an-activity-diagram.md`.
- `NATIVE-033`, `NATIVE-035`, `NATIVE-058..060`, `NATIVE-065..067`, `NATIVE-069`, and `NATIVE-072` now occur in `goals/uml/class/interact-with-a-class-diagram.md`.
- `NATIVE-025` and `NATIVE-026` now occur in `goals/author-in-the-editor/edit-prose.md`.
- `NATIVE-014` now occurs in `goals/read-a-bundle/open-a-bundle.md`.
- `NATIVE-015` now occurs in `goals/read-a-bundle/browse-the-tree.md`.
- `NATIVE-040..044` and `NATIVE-054` now occur in `goals/uml/sequence/interact-with-a-sequence-diagram.md`.
- `NATIVE-049` now occurs in `goals/author-in-the-editor/save-and-undo.md`.
- `NATIVE-008`, `NATIVE-009`, and `NATIVE-055` now occur in `goals/read-a-bundle/use-the-shell.md`.
- `NATIVE-017..020` now occur in `goals/read-a-bundle/work-with-tabs.md`.

# Verification gaps

The following shipped scenarios keep the verification-gap status in their
owning goals. Task 11 adds traceability and does not change their evidence.

- `edit-prose` — native: `NATIVE-047`.
- `save-and-undo` — native: `NATIVE-030`.
- `browse-the-tree` — native: `NATIVE-010..012`.
- `navigate-and-return` — native: `NATIVE-013`.
- `open-a-bundle` — native: `NATIVE-001..005`.
- `read-a-document` — native: `NATIVE-022`, `NATIVE-061`.
- `export-a-bundle` — browser: `BROWSER-001`, `BROWSER-003`, `BROWSER-016`.
- `publish-a-site` — browser: `BROWSER-012`.
- `run-in-a-browser` — browser: `BROWSER-005`.
- `serve-locally` — browser: `BROWSER-002`, `BROWSER-007..010`, `BROWSER-017..020`.
- `share-a-link` — native: `CLI-003`; browser: `BROWSER-015`.
- `command-line-tool` — native: `CLI-008..011`.
- `text-editor-integration` — native: `VSCODE-001`, `VSCODE-008`.
- `interact-with-an-activity-diagram` — native: `NATIVE-037`, `NATIVE-053`.
- `interact-with-a-class-diagram` — native: `NATIVE-033`, `NATIVE-035`, `NATIVE-065..067`, `NATIVE-069`, `NATIVE-072`.
- `interact-with-a-sequence-diagram` — native: `NATIVE-040`, `NATIVE-044`, `NATIVE-054`.
- `sequence-language` — native: all 32 stable `SEQ-*` scenarios.
- `route-the-edges` — native: `NATIVE-051`.
- `select-and-inspect` — native: `NATIVE-052`, `NATIVE-068`, `NATIVE-070`, `NATIVE-071`.
- `solve-the-layout` — native: `NATIVE-036`.

# Open discrepancies

- Task 12 must reconcile the 35 frozen owner paths listed under `# Evidence`.
  This is a traceability ownership correction, not a product-behavior
  discrepancy.

# Feature gaps

- [FG-010](../../../waml/waml-feature-gaps.md#fg-010-—-scenario-to-evidence-traceability)
  remains open because WAML does not automatically check complete links from a
  scenario through a product use case to tests and evidence.
- Specialized actor, use-case, and system-boundary rendering remains separate
  user work. It is not a WAML feature gap and adds no layout constraint to this
  model.
