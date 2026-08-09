# Canonical UML Diagram Types and Use-Case Rendering Design

**Date:** 2026-08-09
**Status:** Approved for implementation planning

## Purpose

WAML shall use explicit, canonical document types for each supported UML
diagram. The editor shall use the document type to select the correct renderer.
Use-case diagrams shall render with UML actor and use-case notation and with a
layout that remains clear for large product models.

This change also adds a reusable `waml upgrade` command. The normal parser
shall reject obsolete document types. The upgrade command shall migrate old
bundles before the normal parser reads them.

## Root cause

WAML currently uses `type: Diagram` for class, domain, ER, and use-case views.
The editor therefore sends use-case views to the class-diagram surface. Actors
and use cases get different accent colors, but they still use class-card
geometry, class-card measurement, generic group rendering, and generic
placement.

The behavioral diagram names are also inconsistent with the names of the
diagram documents. WAML currently uses `uml.Activity`, `uml.StateMachine`, and
`uml.Sequence` as both semantic behavior names and view-document names.

## Canonical diagram document types

WAML shall use these source and wire names:

```text
uml.ClassDiagram
uml.UseCaseDiagram
uml.ActivityDiagram
uml.StateMachineDiagram
uml.SequenceDiagram
```

These names use the standard UML diagram terms. The exact frontmatter strings
are WAML serialization names.

The following old document types shall be invalid in normal parsing:

```text
Diagram
uml.Activity
uml.StateMachine
uml.Sequence
```

Diagnostics for these names shall identify the replacement when it is
unambiguous and shall tell the user to run `waml upgrade`.

The internal model shall keep semantic node types separate from diagram view
types. An activity remains a UML Activity in the semantic model. An
`uml.ActivityDiagram` document is a view that presents that activity. The same
rule applies to state-machine and sequence views.

## Repository migration

The implementation shall migrate all repository-owned documents, fixtures,
templates, examples, generated-output expectations, tests, and documentation
to the canonical names in the same change. New-document actions and CLI
generators shall emit only canonical names.

ER views that currently use `type: Diagram` with an ER profile shall migrate to
`uml.ClassDiagram`. Their profile continues to select ER-specific semantics or
presentation. The diagram type identifies the structural UML view family; the
profile remains an independent extension mechanism.

## `waml upgrade`

### Command surface

```text
waml upgrade [PATH]
waml upgrade [PATH] --check
```

`PATH` defaults to the current directory. The command writes changes by
default, like `waml fmt` and `waml index`. With `--check`, it writes nothing and
returns a non-zero exit code when any migration is pending.

### Migration registry

The CLI shall use an ordered registry of migrations. Each migration has:

- a stable identifier;
- a short description;
- a detector;
- an idempotent source transformation.

The first migration changes legacy diagram document types to the canonical
types. Future language migrations shall use the same registry and execution
pipeline.

### Legacy type migration

The migration reader runs before strict current-language parsing. It reads only
the source structure that a migration requires. It does not make the normal
parser accept obsolete language.

The type mappings are:

```text
uml.Activity     -> uml.ActivityDiagram
uml.StateMachine -> uml.StateMachineDiagram
uml.Sequence     -> uml.SequenceDiagram
```

For a legacy `Diagram` document:

1. If its resolved members include at least one `uml.UseCase` and no
   incompatible classifier type, migrate it to `uml.UseCaseDiagram`.
2. If use cases occur with classes, interfaces, enums, data types, or behavior
   nodes, stop and report an ambiguous legacy diagram. Do not write any file.
3. Otherwise, migrate it to `uml.ClassDiagram`.

Actors, notes, packages, relationships, and empty groups do not make a legacy
diagram ambiguous. An empty legacy `Diagram` migrates to `uml.ClassDiagram`,
which matches the previous default meaning.

### Transaction and validation

The command shall build all candidate file contents in memory. It shall then
validate the complete candidate bundle with the strict current parser and all
normal semantic checks. It shall write only when the complete candidate bundle
is valid.

The write shall use the CLI's atomic multi-file transaction mechanism. A
failure before commit changes no file. A cleanup failure after commit reports a
warning and preserves the committed result.

The command shall report each changed file and each applied migration. A second
run on the upgraded bundle shall report no changes and shall leave all bytes
unchanged.

## Use-case diagram source structure

`uml.UseCaseDiagram` uses the existing `## Members` and `## Layout` grammar.
No new lane or band keyword is required.

Nested member groups author the system boundary and its named horizontal bands:

```markdown
## Members

### External actors
- [Author](../actors/author.md)
- [Reader](../actors/reader.md)

### WAML editor boundary

#### Create and change
- [Edit Prose](../workflows/edit-prose.md)
- [Save and Undo](../workflows/save-and-undo.md)

#### Find and understand
- [Browse the Tree](../workflows/browse-the-tree.md)
- [Select and Inspect](../workflows/select-and-inspect.md)

## Layout
- WAML editor boundary as column with frame
- Create and change as row
- Find and understand as row
- External actors left of WAML editor boundary
```

The parser already represents nested member groups as `DiagramGroup.children`.
The layout grammar already supports row, column, frame, relative placement, and
margin hints.

Use-case validation shall require:

- an external actor group contains only actors, notes, or packages that contain
  actors;
- a system boundary contains use cases and optional notes;
- a named band inside the boundary contains use cases and optional notes;
- an actor is outside the system boundary;
- a use case is inside one system boundary;
- a layout reference resolves to a declared member or group.

Validation shall use resolved element types and group containment. It shall not
infer semantics from English group titles.

## Editor architecture

### Dispatch

Document dispatch shall use the canonical diagram type. It shall not inspect
members to guess the diagram kind. Empty and temporarily invalid use-case
diagrams shall therefore keep the use-case surface and its editor state.

The existing document host, source toggle, inspector, selection model, camera,
and edit actions remain shared.

### Surface structure

The implementation shall extend the current structural diagram canvas through
shared geometry and interaction interfaces. It shall not copy the complete
class-diagram surface.

The surface receives an explicit visual kind. The visual kind selects:

- node measurement;
- node drawing;
- group drawing;
- default placement policy;
- relationship notation.

Shared behavior includes hit testing, selection, dragging, placement preview,
camera control, stale-state display, conflict display, inspector integration,
and source edits.

### Node notation

An actor renders as a UML stick figure with its title below the figure. Its
selection and hit rectangle includes both the figure and the title.

A use case renders as an ellipse with a centered title. The ellipse grows to
fit the measured title and a fixed minimum padding. Long titles wrap to a
bounded number of lines before the ellipse grows wider.

Notes and packages keep their existing notation unless a use-case-specific
rule is required for correct containment.

The measurement result is the single source for solver obstacles, edge ports,
hit testing, selection outlines, focus outlines, and drawing. The renderer
shall not draw outside the measured node rectangle.

### Groups and bands

An actor-only top-level group renders as an unframed external actor rail. Its
name can render as a quiet rail heading.

A top-level group that contains use cases renders as a system boundary frame.
Its authored name renders in the frame heading.

Nested groups inside the system boundary render as horizontal bands in authored
order. Each band has a quiet heading and enough inset for its use cases and
routes. Bands express authored model structure. The renderer shall not create
or name bands that are absent from the source.

### Placement

Authored `## Layout` statements have priority. The use-case default applies
only when required placement is absent:

1. Place external actor groups to the left of system boundaries.
2. Stack actors in stable authored order.
3. Stack named bands vertically in stable authored order.
4. Place use cases inside each band as a balanced row or wrapped grid.
5. Use the relationship graph to reduce crossings without changing authored
   group order.

The solver shall remain deterministic. The same source and viewport produce
the same geometry.

### Relationships

Use-case relationship notation is:

- `associates`: solid line without an arrow;
- `includes`: dashed dependency arrow with `«include»`;
- `extends`: dashed dependency arrow with `«extend»`;
- `specializes`: solid generalization line with a hollow triangle at the
  broader actor or use case.

Routes shall use the measured actor and ellipse boundaries for ports. Labels
shall avoid nodes, boundary headings, band headings, and other labels.

## Error handling

The normal parser shall reject obsolete diagram types. It shall not silently
map them to canonical types.

An invalid use-case member or group produces a semantic diagnostic and keeps
the last valid projection, in line with the editor's current stale-projection
policy.

An ambiguous legacy `Diagram` stops `waml upgrade` before any write. The error
lists the incompatible members and asks the user to set the intended canonical
type.

## Verification

Implementation shall use test-driven development. Tests shall cover:

- parsing and serialization of all five canonical types;
- rejection of all obsolete types with upgrade guidance;
- semantic behavior/view separation in the model;
- CLI migration mapping, ambiguity, idempotence, `--check`, validation, and
  atomic failure;
- migration of repository templates and representative bundles;
- use-case group and band validation;
- actor and use-case measurement;
- group-role classification from resolved types and containment;
- deterministic default placement;
- authored layout precedence;
- relationship line style, marker direction, labels, and ports;
- hit testing and selection for actors and ellipses;
- routing around boundaries, band headings, and labels;
- screenshot regression tests for the real editor, browser/publishing, and
  tooling workflow diagrams at native HiDPI pixels.

The final manual check shall launch each editor with a unique `-Title` slug.
Window captures shall use `scripts/capture-window.ps1` and native pixel output.

## Scope boundaries

This design does not add a new lane keyword, automatic semantic category names,
new use-case relationship kinds, extension-point notation, or a second canvas
interaction system.

The `waml upgrade` framework is in scope. Migrations other than the diagram-type
migration are out of scope for the first implementation.
