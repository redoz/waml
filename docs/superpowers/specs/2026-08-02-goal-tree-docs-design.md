# Goal tree in `docs/waml` — design

Date: 2026-08-02

## Purpose

`docs/waml` describes what WAML *is* (architecture, concepts, views). It does
not say what WAML is *for*, what is finished, or what remains before the
product is worth calling shipped. This design adds a goal tree: a root goal
decomposed until the leaves are implementation-sized, each leaf carrying a
status and an MVP flag.

The tree exists so work can be *filled in* rather than invented. A complete cut
of every intended feature — including features nobody has started — is more
valuable than an accurate list of what exists today, because it makes "done"
a reachable state instead of a mood.

## Root goal

> WAML is a native documentation tool. UML support is its first big feature.

The tool may later grow into an internal documentation or wiki tool. That
possibility shapes boundaries (nothing in the core may assume UML) but is not
a goal to be delivered.

## MVP definition — the dogfood bar

MVP is reached when `docs/waml` itself can be authored and read entirely in the
native editor — no text editor in the loop — and shared as a link that a reader
opens without installing anything.

Every leaf goal carries `MVP: yes` or `MVP: no` measured against that sentence.
A leaf can be `done` and `MVP: no`; a leaf can be `planned` and `MVP: yes`. The
two axes are independent and both are needed.

## Format

Plain OKF Markdown with links. No new frontmatter types yet. A later pass may
promote each goal to a typed concept once the tree has proven its shape; the
file layout below is chosen so that promotion is additive.

### `index.md` is generated, not authored

`waml::index_md::render_index` emits exactly an H1, an optional description
paragraph, and a flat member list. `reindex_source` rewrites every directory
index from the model. Anything else placed in an `index.md` is drift that a
future reindex will discard.

Therefore: **no tables, no prose sections, no status in any `index.md`.** All
authored payload lives in ordinary concept documents beside the index.

Note for later cleanup, out of scope here: `docs/waml/architecture/index.md`
already carries hand-written `## Understand the model` sections and is drift by
this rule. `reindex_source` currently has no product caller — only tests — so
nothing regenerates these indexes today.

### Layout

```
docs/waml/goals/
  index.md                     members only
  root-goal.md                 root statement, MVP definition, status legend,
                               level-1 roadmap table
  read-a-bundle/
    index.md
    <goal>.md
  author-in-the-editor/
    index.md
    <goal>.md
  trust-the-content/
    index.md
    <goal>.md
  share-and-publish/
    index.md
    <goal>.md
  tooling-around-the-repo/
    index.md
    <goal>.md
  uml/
    index.md
    class/          index.md, feature-cut.md, <leaf>.md
    sequence/       index.md, feature-cut.md, <leaf>.md
    activity/       index.md, feature-cut.md, <leaf>.md
    state-machine/  index.md, feature-cut.md, <leaf>.md
    use-case/       index.md, feature-cut.md, <leaf>.md
    shared/         index.md, <goal>.md   (layout, edge routing, labels,
                                           selection, inspector)
  beyond-uml.md                horizon; explicitly not MVP
```

`docs/waml/index.md` gains a `Goals` member entry.

### Goal document template

```markdown
# <Title>

**Goal:** one sentence, in the user's voice.

**Why:** what breaks or stays annoying without it.

**Done when:** a testable condition. Not "improve X".

**Status:** done | partial | planned | horizon
**MVP:** yes | no

## Sub-goals
- [Child](./child.md) — status, one clause

## Notes
Free text. Evidence, links to concepts, known traps.
```

A leaf has `## Notes` and no `## Sub-goals`. A leaf is sized so one
implementation plan finishes it.

### Status vocabulary

- `done` — the "Done when" condition holds today and is covered by a test or a
  reproducible manual check.
- `partial` — some of it works; the "Done when" condition does not hold.
- `planned` — wanted, not started.
- `horizon` — wanted eventually, deliberately unscheduled. Never `MVP: yes`.

Rows written in the first pass are marked `unverified` alongside their guessed
status. A later audit pass replaces each guess with an evidence-backed status
and, for `done`/`partial`, a `file:line` citation. The `unverified` marker is
the audit's own to-do list.

## Level-1 goals

| Goal | First-pass state |
| --- | --- |
| Read a bundle | mostly done — tree, preview tabs, document and diagram views, navigation history |
| Author in the editor | partial — model edits, save, undo, savepoints; text authoring weaker |
| Trust the content | partial — lossless syntax, broad tests; diagnostics not aggregated across layers (`issues.md` P1) |
| UML | partial — see the five kind cuts |
| Share and publish | mostly done — share link, wasm build, GitHub Pages |
| Tooling around the repo | mostly done — CLI, language server, VS Code extension; MVP-optional |
| Beyond UML | horizon |

Layout solving, edge routing, and diagram chrome live under `uml/shared/`
rather than as a level-1 goal: they exist to serve the five kinds and have no
independent user-facing "done".

## The UML cut

Five kinds, each with a `feature-cut.md` holding one row per intended feature:

| Column | Meaning |
| --- | --- |
| Feature | the smallest nameable thing a reader would ask for |
| Status | done / partial / planned / horizon |
| MVP | yes / no |
| Evidence | `file:line`, test name, or `unverified` |
| Leaf | link to a goal document when the row needs breakdown |

Known model surface today, from `crates/waml/src/model.rs`:

- `UmlMetaclass`: Class, Interface, Enum, DataType, Package, Note,
  Association, Actor, UseCase, InstanceSpecification
- `RelationshipKind`: associates, aggregates, composes, specializes,
  implements, depends, annotates, includes, extends, instance of, links
- `BehaviorKind`: Activity, StateMachine, Sequence

Expected row counts and coverage per kind:

- **class** (~25): classifiers, members, visibility, static and abstract,
  generics, all eleven relationship kinds, association ends and multiplicity,
  association class, packages, notes, stereotypes and profiles, instance
  specifications.
- **sequence** (~20): lifelines, activations, synchronous and asynchronous
  messages, replies, create and destroy, self-message, combined fragments
  (alt, opt, loop, par, critical, ref), guards, gates, time constraints.
- **activity** (~20): initial, final, flow final, actions, decision, merge,
  fork, join, object nodes, pins, partitions and swimlanes, exception
  handling, expansion regions, signal send and receive.
- **state-machine** (~15): states, transitions, guards, effects, entry and
  exit and do behaviors, initial and final, choice, junction, composite and
  submachine states, history, internal transitions.
- **use-case** (~10): actors, use cases, system boundary, associations,
  includes, extends, actor generalization.

State machine is a real kind, not a stub: `uml.StateMachine` parses states and
transitions with guards and effects, solves through the flow solver with
`FlowFlavor::StateMachine`, renders in `behavior_doc_view.rs`, and has golden
and property tests. Its cut is expected to land mostly `done`/`partial`. It is
`MVP: no` regardless — the dogfood bar does not need it.

Use case is its own kind rather than rows inside class, even though Actor and
UseCase are structural metaclasses today. `MVP: no`.

## Non-goals

- No new frontmatter types, no `uml.Goal` stereotype, no profile work.
- No diagram or view of the goal tree in this pass.
- No repair of the existing `architecture/index.md` drift.
- No code change of any kind. This is documentation only.
- No audit pass. Statuses are first-pass guesses marked `unverified`.

## Risks

- **The cut goes stale.** Mitigated by keeping rows coarse — a row names a
  feature, not an implementation detail — and by the `unverified` marker making
  the trust level visible rather than implied.
- **Row counts balloon.** If a kind's cut exceeds roughly thirty rows, the kind
  is under-decomposed and should gain sub-cuts rather than a longer table.
- **`index.md` discipline erodes.** Anyone adding a table to an index breaks
  the generated-index contract silently, since nothing regenerates indexes
  today. The template and this document are the only guard.
