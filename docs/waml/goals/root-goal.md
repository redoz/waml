# Root Goal

**Goal:** WAML is a native documentation tool. UML support is its first large
feature.

**Why:** Documentation that is not in the repository becomes incorrect.
Documentation that is in the repository needs a tool to read it. WAML is that
tool. It reads and writes plain Markdown that a reviewer can diff. It draws
that Markdown as documents and diagrams that a reader can navigate.

**Done when:** The MVP bar below is true. Also, the five UML kinds have a
complete feature cut, and no `planned` row in those cuts is `MVP: yes`.

**Status:** partial
**MVP:** yes

## The MVP bar

The MVP is complete when an author can write and read `docs/waml` fully in the
native editor, with no text editor, and can send it as a link that a reader
opens with no installation.

Each goal has the flag `MVP: yes` or `MVP: no`. The flag records one thing
only: whether the goal is necessary for the sentence above. The bar is small
and self-referential on purpose. The first user of WAML is the documentation of
WAML.

[The MVP Definition](./mvp.md) consolidates this bar into one page set: the
scope with its out-of-scope list, the completeness matrix for each area, the
ordered gap backlog, the explicit deferrals, and the definition of done for
each area. That document orders the work. This tree stays the source of truth
for each single goal.

## Status legend

| Status | Meaning |
| --- | --- |
| `done` | The "Done when" condition is true. A test or a manual check shows this. |
| `partial` | Part of the goal operates. The "Done when" condition is not true. |
| `planned` | The team wants the goal. Work has not started. |
| `horizon` | The team wants the goal later. It is not scheduled. It is never `MVP: yes`. |

`Status` and `MVP` are independent. A goal can be `done` and `MVP: no`. A goal
can be `planned` and `MVP: yes`. The second combination shows the work that
blocks the bar.

## Level-1 roadmap

| Goal | Status | MVP | Note |
| --- | --- | --- | --- |
| [Read a Bundle](read-a-bundle/) | partial | yes | Seven reading goals are done. Reading every diagram kind remains planned. |
| [Author in the Editor](author-in-the-editor/) | partial | yes | Prose editing, save, and undo are done. Document, model, and canvas authoring remain partial. |
| [Trust the Content](trust-the-content/) | partial | yes | Round-trip and canonical formatting are done. Diagnostics, references, and index correction remain partial. |
| [UML](uml/) | partial | yes | Diagram interaction is shipped. The feature cuts and shared presentation goals remain partial or planned. |
| [Share and Publish](share-and-publish/) | partial | yes | Share and export are done. Browser parity and publication remain partial. |
| [Tooling Around the Repo](tooling-around-the-repo/) | partial | no | The command-line tool is done. The language server and VS Code integration remain partial. |
| [Beyond UML](./beyond-uml.md) | horizon | no | A general documentation tool and wiki. |

## Notes

- Layout, edge routing, and diagram chrome are not a level-1 goal. They give
  service to the five UML kinds. They have no "done" that a user can see. Thus
  they are in [UML shared](uml/shared/).
- These documents are the source of truth. The implementation plans and the
  design specifications in `docs/superpowers/` are records of past work. They
  show a decision at one time. They do not show current intent. If a plan and
  this tree do not agree, use this tree. Then correct the product or correct
  this tree.
- Each goal must state its intent and its behavior with sufficient accuracy for
  a person to write a test from the text alone. "Done when" is not an opinion.
  If a behavior needs more than one sentence, put it in a document adjacent to
  the goal, as the [Sequence Language](uml/sequence/language.md) does for its
  cut. Then the goal points to that document.
- A behavior in this tree that has no test is an intention. It is not a
  guarantee. Write it as an intention.
- The best form of that rule is a scenario: `Given`, `When`, `Then`, with an
  identifier that a test names. The [Sequence
  Language](uml/sequence/language.md) has the first set. Other goals get
  scenarios when their behavior becomes stable. A scenario for an unstable
  behavior gives no help.
- All documents in this tree use ASD-STE100 Simplified Technical English: one
  idea in each sentence, an active verb, the present tense, and the same word
  for the same thing.
- Each `index.md` in this tree is generated content. It has an H1, one
  description paragraph, and a list of members. Put all other content in a
  document adjacent to the index. Do not put it in the index.
