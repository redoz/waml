# Markdown editor application integration — design

**Date:** 2026-07-31
**Status:** Approved in conversation; written-spec review pending
**Sequence:** 4 of 4
**Depends on:** Incremental Markdown syntax platform; Markdown editor
foundation; Markdown presentation and motion

## Problem

The native Source view currently wraps Makepad's read-only `Markdown` widget and
receives raw bundle text during synchronization. Editing must become part of
WAML's document transaction, analysis, persistence, diagnostics, canvas, and
LSP architecture without introducing a second source authority or allowing
stale results to overwrite newer keystrokes.

## Goal

Replace the read-only Source surface with the WAML-owned Markdown editor and
integrate it using the same snapshot model as Roslyn-based editors:

- the editor text buffer publishes versioned source changes;
- the application workspace publishes immutable document/session snapshots;
- syntax and semantic analyses derive incrementally from previous snapshots;
- every result is revision-checked;
- native editor and LSP consume the same analysis products.

## Roslyn correspondence

The intended ownership analogy is:

| WAML | Visual Studio / Roslyn |
|---|---|
| `MarkdownDocumentSession` | editor text buffer and text snapshots |
| `SourceText` + document revision | `SourceText` / `ITextSnapshot` |
| application `EditorSession` | `Workspace` |
| immutable session snapshot | immutable `Solution` snapshot |
| WAML document snapshot | Roslyn `Document` |
| incremental green tree | `SyntaxTree.WithChangedText` |
| canvas, diagnostics, LSP | semantic and editor consumers |

This is an architectural correspondence, not an API emulation.

## Application snapshot ownership

Evolve `waml-editor::EditorSession` to publish an immutable
`EditorSessionSnapshot` containing:

- session revision;
- source bundle and document catalog;
- Markdown syntax snapshots;
- WAML semantic analyses and diagnostics;
- dependency and affected-document information.

The mutable session coordinates transitions between snapshots; consumers hold
immutable snapshots. An older snapshot remains valid for readers but can never
be installed as current after a newer revision.

Raw source is authoritative. Syntax and semantic products are derived,
revisioned projections.

## Source view ownership

`SourceView` owns the live `MarkdownDocumentSession` for its immutable document
identity. It is responsible for:

- synchronizing from the active application snapshot;
- presenting the WAML Markdown editor widget;
- emitting source-edit intents;
- forwarding navigation intents;
- preserving view-local selection and scroll state;
- applying diagnostics and semantic presentation updates.

`App` and `DocumentHost` do not inspect Markdown editor internals or special-case
source rendering. They route typed outcomes through the existing view/session
boundaries.

## Edit transaction

```text
keyboard / pointer / IME
  -> MarkdownDocumentSession applies exact local edit
  -> new local text + syntax snapshot is drawn immediately
  -> SourceView emits ProposedSourceEdit {
       document, base_revision, changes, syntax_update
     }
  -> EditorSession validates the base revision
  -> source bundle and document revision advance
  -> the proposed syntax update is promoted without reparsing
  -> incremental WAML semantic analyses run from prior snapshots
  -> EditorSession publishes revision-tagged result
  -> DocumentHost applies result only if it is still current
  -> SourceView, diagnostics, canvas, navigation, and status projections refresh
```

The source change is not rolled back merely because diagnostics exist.

## Incremental analysis scheduling

The syntax update needed for immediate source presentation runs as part of the
edit transaction using the exact `TextChange`. After validating the base
revision and resulting source identity, the application promotes that same
immutable update. It never repeats Markdown parsing for the accepted revision.

Semantic/domain analysis is scheduled for every revision without a correctness
debounce. It may run off the UI thread against immutable snapshots. Completion
is accepted only when its source revision is still current; stale completion is
discarded.

Unchanged documents, greens, Markdown blocks, WAML language islands, and
semantic dependencies are reused. If an edited island cannot lower, that island
reports diagnostics and retains only its previous projection, drawn desaturated
with an explicit stale marker. Unrelated canvas projections remain current.
Once the island lowers again, its current projection replaces the marked
fallback.

An internal analysis failure may retain the previous semantic projection as a
clearly diagnosed fallback, but source and syntax still advance.

## Persistence and dirty state

Every accepted raw source edit:

- advances the session revision;
- marks that exact revision dirty;
- schedules the existing save mechanism;
- persists literal source, including temporarily invalid text.

A save marks only the revision it actually wrote as clean. Completion of an
older save cannot clear a newer dirty revision.

Native and browser persistence adapters remain application responsibilities.
The Markdown widget never writes files or URLs directly.

## External reloads and conflicts

An external document replacement carries a source revision and document
identity.

- If the editor has no newer local revision, install the replacement and map
  selection/scroll through the change where possible.
- If unsaved local edits exist, route the conflict through the application's
  existing conflict policy rather than silently replacing the buffer.
- Initial load and accepted external replacement cut motion directly to target
  geometry.

## Diagnostics and canvas behavior

Syntax diagnostics update from the newest Markdown snapshot. Semantic
diagnostics update from the newest accepted analysis snapshot. Each diagnostic
retains document identity, revision, and source range.

The canvas refreshes only affected semantic projections. A temporarily invalid
island cannot erase unrelated nodes or cause whole-canvas flicker. Diagnostic
navigation activates the source tab, maps the diagnostic range to the current
snapshot, and selects it.

## Navigation

- Normal clicks place or extend the caret.
- Ctrl/Cmd-click on a parsed link emits a typed navigation intent.
- Relative document and asset links resolve from the active bundle path.
- WAML symbol links route through the existing navigation service.
- Unsafe or unsupported targets produce a non-destructive status diagnostic.

## LSP integration

The in-app editor calls syntax and semantic APIs directly. It does not communicate
with its own LSP server.

The existing WAML LSP adapter consumes the same immutable snapshots to provide:

- Markdown and WAML diagnostics;
- document symbols and WAML navigation;
- semantic tokens for markers, headings, links, code, and embedded languages;
- link targets;
- completion where a concrete WAML producer exists.

LSP positions convert through the shared `LineIndex`. Revision checks prevent
responses computed for an older document version from being published as
current.

## Rollout

1. Add the new editor surface behind the existing `SourceView` boundary.
2. Characterize current tab identity, raw-source lookup, link navigation,
   scrolling, and canvas occlusion.
3. Route read-only synchronization through immutable snapshots.
4. Enable editing and exact source-edit outcomes.
5. Connect incremental diagnostics and affected canvas refresh.
6. Connect persistence, external reloads, and conflict behavior.
7. Switch LSP Markdown consumers to the shared snapshot queries.
8. Remove the upstream Makepad `Markdown` runtime path and obsolete helper code.

There is one production source-editor path at the end of every merged rollout
stage. Temporary feature flags may gate activation, but no long-lived competing
parser or editor implementation is permitted.

## Error handling

- Stale source edit: reject with current revision and rebase view state.
- Stale analysis/save result: discard without changing current state.
- Incremental parser fallback: accept the full result and record the reason.
- Syntax/semantic diagnostics: preserve source and continue editing.
- Persistence failure: retain dirty state and use existing user-visible error
  reporting.
- Missing source document: show an editable-disabled diagnostic state rather
  than stale content from another tab.
- Navigation failure: leave selection unchanged and report status.

## Testing

### Session and revision tests

- Exact edit advances source and document/session revisions once.
- Stale base revision is rejected.
- Unchanged documents and analyses retain identity.
- Stale syntax, semantic, and save completions are ignored.
- An older save cannot clear a newer dirty revision.
- Invalid source remains the persisted canonical source.

### View integration

- Source tab loads the correct nested bundle path.
- Editing updates the current source tab without losing selection or scroll.
- Tab switching and closing preserve the existing `DocumentHost` contracts.
- Normal click edits; Ctrl/Cmd-click navigates.
- Missing source cannot display prior-tab content.
- External reload mapping and dirty conflict routing.

### Analysis and canvas

- One-character edits reparse the expected Markdown range/island.
- Unaffected canvas projections retain identity and geometry.
- Invalid edited islands report diagnostics without whole-canvas flicker.
- Returning to valid source updates the affected projection.
- Diagnostic navigation selects the correct current source range.

### Persistence and LSP

- Native/browser adapters receive the exact current raw source.
- LSP diagnostics and semantic tokens match in-app classification for one
  snapshot.
- UTF-8 source offsets and LSP UTF-16 positions round-trip.

### Verification

- Focused crate tests after each rollout stage.
- `cargo fmt --check`.
- `cargo test --workspace`.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`.
- Native HiDPI screenshots and motion captures using the repository capture
  script.

## Success criteria

- The native Source view is editable and uses literal Markdown as its authority.
- Every edit travels as exact revisioned text changes.
- Analysis, canvas, diagnostics, persistence, and LSP agree on snapshot identity.
- Incremental results reuse unaffected syntax and semantic state.
- Stale work cannot overwrite newer source or clear newer dirty state.
- The upstream Makepad `Markdown` widget is absent from the production path.
