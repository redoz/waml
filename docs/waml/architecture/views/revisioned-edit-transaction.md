---
type: uml.SequenceDiagram
title: Revisioned Edit Transaction
description: The document-revision, session-revision, preparation, commit, and stale-completion paths for edits.
---

# Revisioned Edit Transaction

## Notes
- [Editor Session](../concepts/implementation/editor-session.md) owns the immutable live snapshot and its session revision.
- A Markdown proposal carries a `DocumentRevision`. A source installation advances the separate editor `session_revision`.
- Semantic completion compares both `session_revision` and the installed source `Arc` identity before it replaces analysis state.
- An exact Markdown edit has a source phase and a semantic-completion phase. A semantic failure does not roll back accepted source text.
- The production app runs semantic preparation and completion in one call stack. The completion boundary still rejects a stale result.
- A semantic operation prepares all candidate state before one atomic snapshot replacement. A failure keeps the old snapshot and history unchanged.
- Affected identities describe semantic impact. The shell prepares every open document and synchronizes the active view after an analysis change.
- [FG-009 — component ports and transactions](../../waml-feature-gaps.md#fg-009-—-component-ports-and-transactions) records why the separate completion boundary and compare-and-swap guards are messages and prose notes.

## Lifelines
- [Author](../concepts/workflows/author.md) as author
- [App Shell](../concepts/implementation/app-shell.md) as shell
- [Markdown Editor](../concepts/implementation/markdown-editor.md) as markdown
- [Editor Session](../concepts/implementation/editor-session.md) as session
- [Editor Session](../concepts/implementation/editor-session.md) as snapshot
- [Source Bundle](../concepts/implementation/source-bundle.md) as source
- [Editing and Round Trip](../concepts/workflows/editing-and-round-trip.md) as lowering
- [Prepared Candidate](../concepts/implementation/prepared-candidate.md) as preparation
- [Document Host](../concepts/implementation/document-host.md) as documents
- [Diagram Renderer](../concepts/implementation/diagram-renderer.md) as diagrams

## Messages
- author calls shell `submit exact Markdown changes at a DocumentRevision`
- shell calls markdown `validate changes and build a ProposedSourceEdit`
- markdown returns `proposal with base and successor DocumentRevision` to shell
- shell calls session `promote the exact source proposal`
- session calls snapshot `compare DocumentRevision and accepted SourceText identity`
- alt
  - when `the document revision is stale or the source identity differs`
    - snapshot returns `mismatch` to session
    - session returns `reject; old snapshot intact` to shell
    - shell returns `stale edit result` to author
  - else
    - snapshot returns `match` to session
    - session calls lowering `apply ExactSourceEdit and build its inverse`
    - lowering calls source `apply exact changes to immutable source data`
    - source returns `candidate source` to lowering
    - lowering returns `candidate source and inverse edit` to session
    - session calls snapshot `install source at the next session_revision`
    - snapshot returns `source-only snapshot and semantic request` to session
    - session returns `source change and semantic request` to shell
    - shell calls preparation `prepare promoted Markdown semantics`
    - preparation returns `semantic completion or document diagnostic` to shell
    - shell calls session `install semantic completion`
    - session calls snapshot `compare completion session_revision and source Arc identity`
    - alt
      - when `completion is stale`
        - snapshot returns `mismatch` to session
        - session returns `ignore stale completion` to shell
      - else
        - snapshot returns `match` to session
        - session calls snapshot `install semantic analysis on current source`
        - snapshot returns `atomic semantic snapshot` to session
        - session returns `affected documents and diagrams` to shell
        - shell calls documents `prepare and reconcile every open document`
        - documents calls diagrams `synchronize the active diagram when open`
        - diagrams returns `active diagram synchronized` to documents
        - documents returns `open documents reconciled` to shell
    - shell returns `current source and semantic status` to author
- author calls shell `submit a semantic edit`
- shell calls session `apply semantic edit against the current snapshot`
- session calls snapshot `read current source and session_revision`
- snapshot returns `immutable installed state` to session
- session calls lowering `lower edit to candidate source and inverse edit`
- lowering returns `candidate source and inverse edit` to session
- session calls preparation `prepare_candidate(candidate_source, previous, next session_revision)`
- alt
  - when `preparation fails`
    - preparation returns `analysis error` to session
    - session returns `reject; old snapshot and history intact` to shell
    - shell returns `previous view and diagnostics` to author
  - else
    - preparation returns `PreparedCandidate` to session
    - session calls snapshot `install prepared snapshot atomically`
    - snapshot returns `new immutable snapshot` to session
    - session returns `committed change` to shell
    - shell calls documents `prepare and reconcile every open document`
    - documents calls diagrams `synchronize the active diagram when open`
    - diagrams returns `active diagram synchronized` to documents
    - documents returns `open documents reconciled` to shell
    - shell returns `updated view` to author
