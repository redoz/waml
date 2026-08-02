# Root Goal

**Goal:** WAML is a native documentation tool. UML support is its first big
feature.

**Why:** Documentation that lives outside the repository rots. Documentation
that lives inside the repository is unreadable without a tool. WAML is that
tool: it reads and writes plain Markdown a reviewer can diff, and it draws that
Markdown as documents and diagrams a reader can navigate.

**Done when:** The MVP bar below holds, and the five UML kinds have a complete
feature cut with no `planned` row marked `MVP: yes`.

**Status:** partial
**MVP:** yes

## The MVP bar

MVP is reached when `docs/waml` itself can be authored and read entirely in the
native editor — no text editor in the loop — and shared as a link that a reader
opens without installing anything.

Every goal below carries `MVP: yes` or `MVP: no` measured against that
sentence, and nothing else. The bar is deliberately small and deliberately
self-referential: the first real user of WAML is WAML's own documentation.

## Status legend

| Status | Meaning |
| --- | --- |
| `done` | The "Done when" condition holds and a test or reproducible manual check covers it. |
| `partial` | Some of it works. The "Done when" condition does not hold. |
| `planned` | Wanted, not started. |
| `horizon` | Wanted eventually, deliberately unscheduled. Never `MVP: yes`. |

`Status` and `MVP` are independent. A goal can be `done` and `MVP: no`. A goal
can be `planned` and `MVP: yes` — those are the ones that block the bar.

Rows and goals written in the first pass carry `unverified` beside their
status. That marker means the status is a reading of the code from memory, not
an audit. A later pass replaces each guess with an evidence-backed status and a
`file:line` citation. The set of remaining `unverified` markers is that pass's
to-do list.

## Level-1 roadmap

| Goal | Status | MVP | Note |
| --- | --- | --- | --- |
| [Read a Bundle](read-a-bundle/) | partial | yes | Tree, preview tabs, document and diagram views, navigation history all exist. |
| [Author in the Editor](author-in-the-editor/) | partial | yes | Model edits, save, undo, savepoints exist. Prose authoring and canvas ergonomics are the weak side. |
| [Trust the Content](trust-the-content/) | partial | yes | Lossless syntax and broad tests exist. Diagnostics are not aggregated across layers. |
| [UML](uml/) | partial | yes | Class and the behavior kinds render. The cut is incomplete in every kind. |
| [Share and Publish](share-and-publish/) | partial | yes | Share link, wasm build, and Pages publish exist. |
| [Tooling Around the Repo](tooling-around-the-repo/) | partial | no | Command-line tool, language server, and VS Code extension exist. None is needed for the bar. |
| [Beyond UML](./beyond-uml.md) | horizon | no | A general documentation and wiki tool. |

## Notes

- Layout solving, edge routing, and diagram chrome are not a level-1 goal. They
  serve the five UML kinds and have no independent user-facing "done", so they
  live under [UML shared](uml/shared/).
- The wiki horizon shapes boundaries rather than work: nothing in the core may
  assume UML. It is not a goal to be delivered.
- Every `index.md` in this tree is generated content — an H1, a description,
  and a member list. Authored payload belongs in a document beside the index,
  never in the index itself.
