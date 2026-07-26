# Editor Architecture Ownership — Design

**Date:** 2026-07-27
**Status:** Approved in conversation; written-spec review pending
**Branch:** `codex/app-action-coordinator`

## Problem

`App` is simultaneously the document transaction boundary, tab controller,
document-view registry, popup presenter, global UI controller, and event-priority
policy. Its roughly 710-line `handle_actions` method encodes event priority
through source order and early returns. Callers also repeat tab mutation,
tab-strip refresh, view reconciliation, and active-view synchronization.

The existing `DocView` seam does not yet transfer document-view authority:

- `App` special-cases `TabKind::Source` during synchronization.
- `App` downcasts `dyn DocView` to `ClassDiagramView` to set identity and refresh
  after model mutation.
- chrome and accent queries construct temporary views instead of consulting the
  registered live view.
- unused outcome fields were added in anticipation of callers that never
  arrived.

The result is a syntactically indirect tagged union around a still-central
application object. Adding a view or mutation path requires shell edits in
multiple places.

## Goal

Transfer ownership rather than merely move code:

1. `EditorSession` owns document data and the one apply/rebuild/dirty
   transaction.
2. `DocumentHost` owns tabs, live views, tab transitions, reconciliation, and
   active-view synchronization.
3. Each live `DocView` owns its identity, content synchronization, chrome,
   accent, action handling, and post-mutation refresh.
4. `App::handle_actions` becomes a short coordinator whose explicit phases
   preserve the current action priority.

## Hard constraints

- No intentional visual change.
- No interaction, shortcut, popup, overlay, dock, tab, startup, or save-timing
  change.
- Preserve the current event priority, including actions that intentionally
  continue through the batch rather than consuming it.
- Preserve one shared Makepad body widget surface; this change does not mount a
  widget subtree per tab.
- Preserve the existing `Box<dyn DocView>` extension seam. A single composition
  root may map `TabKind` to a concrete constructor; no other shell code may know
  concrete view types.
- No new dependencies.
- Native persistence remains outside this change. The session may accurately
  retain dirty state, but this refactor must not imply that the current native
  no-op backend durably saved anything.

## Chosen approach

Implement the ownership transfer in reviewable, compiling stages on one branch.
Do not perform a big-bang rewrite and do not replace `DocView` with a concrete
view enum.

A concrete enum would be simpler for three current views, but it would retain
the central type authority this work is meant to remove. A big-bang rewrite
would make event-order and UI regressions difficult to localize. Staged
replacement lets characterization tests protect each boundary before callers
move.

## Architecture

```text
Actions
  |
  v
App action coordinator
  |-- unconditional phase: caption controls, popup results, conflict-list effects
  `-- exclusive phase: ordered handlers returning Continue / Consumed
          |
          +--> DocumentHost --------> active DocView
          |      tabs + views          document-local intent
          |
          `--> EditorSession
                 bundle + model + revision + dirty state
                         |
                         v
                  SessionChange
                         |
                         v
              DocumentHost invalidates the affected live view
              App refreshes global projections named by the change
```

The boundaries are deliberately asymmetric:

- `EditorSession` is framework-light and knows nothing about `Cx`, widgets,
  tabs, popups, or views.
- `DocumentHost` is editor-UI infrastructure. It knows tabs, live views, and the
  shared body surface, but not project-tree, recents, overlays, or persistence
  backends.
- `DocView` reads session data and emits document-local intents. It does not
  mutate the session, tabs, or popup root directly.
- `App` remains the shell composition root and owns permanent chrome and
  platform integration.

## Unit 1 — `EditorSession`

Create `crates/waml-editor/src/editor_session.rs`.

```rust
pub struct EditorSession {
    bundle: Vec<(String, String)>,
    model: waml::model::Model,
    revision: u64,
    dirty_revision: Option<u64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SessionChange {
    pub revision: u64,
    pub model_changed: bool,
    pub source_changed: bool,
    pub navigation_changed: bool,
    pub conflicts_changed: bool,
}

impl EditorSession {
    pub fn replace(
        &mut self,
        bundle: Vec<(String, String)>,
        model: waml::model::Model,
    ) -> SessionChange;

    pub fn apply_ops(
        &mut self,
        ops: &[waml::ops::Op],
    ) -> Result<SessionChange, waml::ops::OpError>;

    pub fn bundle(&self) -> &[(String, String)];
    pub fn model(&self) -> &waml::model::Model;
    pub fn revision(&self) -> u64;
    pub fn is_dirty(&self) -> bool;
    pub fn mark_saved(&mut self, revision: u64);
}
```

`apply_ops` is atomic:

1. Apply against the current bundle into a new bundle.
2. On error, leave bundle, model, revision, and dirty state unchanged.
3. On success, rebuild the model once, increment revision once, mark that
   revision dirty, and return one `SessionChange`.

All existing mutation producers, including placement-dial `PlaceSet` and
conflict-list `PlaceRm`, call this method. `App` schedules the existing Makepad
save timer after a successful dirty change because timers and platform save
backends require `Cx`. The session owns whether a revision is dirty; the shell
owns when and how a save is attempted.

`replace` is the single open/reload transaction. It increments revision and
returns full invalidation, but establishes the loaded revision as clean.

`SessionChange` is intentionally explicit rather than a generic event bus or
bitflags dependency. All current WAML operations rebuild the full model, so all
four invalidation booleans are initially true after a successful apply. The
shape allows future narrowing without changing callers.

## Unit 2 — `DocumentHost`

Create `crates/waml-editor/src/document_host.rs`.

```rust
pub struct DocumentHost {
    tabs: OpenTabs,
    views: HashMap<LiveId, Box<dyn DocView>>,
}

pub enum DocumentCommand {
    Open {
        key: String,
        title: String,
        node_kind: TreeKind,
        persistent: bool,
    },
    OpenSource {
        key: String,
        title: String,
    },
    Activate(LiveId),
    Promote(LiveId),
    PromoteSubject(String),
    Close(LiveId),
}
```

One public transition method owns the choreography:

```rust
pub fn transition(
    &mut self,
    cx: &mut Cx,
    ui: &WidgetRef,
    session: &EditorSession,
    command: DocumentCommand,
) -> bool;
```

For every command it:

1. mutates `OpenTabs`;
2. removes replaced or closed live views;
3. creates any missing live view from the resulting `DocTab`;
4. calls deactivate/activate when the active id changes;
5. refreshes the tab widget and queries accent from the registered active view;
6. synchronizes the active view and its declared chrome.

The method returns whether the tab state changed. Callers never sequence
`open_*`, `refresh_doc_tabs`, `reconcile_views`, and `sync_active_tab`
themselves.

Initial bundle opening may seed `OpenTabs` through a dedicated
`replace_for_session` method because it replaces the complete document set
rather than performing a user tab command. That method still runs the same
reconcile/refresh/sync tail.

`DocumentHost` exposes read-only tab queries needed by shell chrome, such as
`active_tab`, `tabs`, and document-switcher items. It does not expose mutable
`OpenTabs`.

## Unit 3 — live `DocView` authority

Replace the current trait contract with a view-data context:

```rust
pub struct ViewData<'a> {
    pub model: &'a Model,
    pub bundle: &'a [(String, String)],
    pub revision: u64,
}

pub trait DocView {
    fn sync(&mut self, cx: &mut Cx, body: &BodyWidgets, data: ViewData<'_>);

    fn handle(
        &mut self,
        cx: &mut Cx,
        body: &BodyWidgets,
        actions: &Actions,
        data: ViewData<'_>,
    ) -> ViewOutcome;

    fn after_session_change(
        &mut self,
        cx: &mut Cx,
        body: &BodyWidgets,
        data: ViewData<'_>,
        change: SessionChange,
    ) {
        self.sync(cx, body, data);
    }

    fn chrome(&self) -> BodyChrome;
    fn tab_accent(&self) -> Option<Vec4> { None }
    fn on_activate(&mut self, cx: &mut Cx, body: &BodyWidgets) {}
    fn on_deactivate(&mut self, cx: &mut Cx, body: &BodyWidgets) {}
}
```

Each constructor receives and stores the immutable identity it needs:

- `ClassDiagramView`: diagram key and title.
- `ClassifierPreviewView`: classifier key and node kind.
- `SourceView`: classifier key and node kind.

Consequences:

- `ClassDiagramView::set_active`, `as_any_mut`, and `downcast_diagram` disappear.
- `SourceView::sync` resolves raw source from `ViewData.bundle` and feeds the
  Markdown widget itself. `App` has no `TabKind::Source` branch.
- `ClassDiagramView::after_session_change` performs the existing
  camera-preserving `update_scene` refresh.
- Other views use the default full synchronization unless a narrower refresh
  is behaviorally required.
- `BodyChrome` and tab accent are queried from the registered live active view.
  `body_chrome(active_tab)` and `tab_accent(active_tab)` no longer construct
  throwaway views.
- Only the composition-root factory matches `TabKind`.

The right-dock state remains app-global exactly as it is now. A view declares
whether it has a right dock and its icon through `BodyChrome`; it does not own
the global dock open/closed state.

## Unit 4 — view outcomes and mutation refresh

Keep the outcome mechanism because it has real producers and consumers:

- `ops`
- `popup`
- `promote_subject`
- `close_active`
- `statusbar_dirty`

Delete the unused `open_preview` and `open_right_dock` fields, their tests, and
their relay branches. Reintroducing either later requires a real producing
interaction and its behavioral test.

Rename `relay_outcome` to `apply_view_outcome`. Its responsibilities are:

1. apply ops through `EditorSession`;
2. ask `DocumentHost` to refresh the active live view through
   `after_session_change`;
3. present popup requests through `popup_root`;
4. route tab intents through `DocumentHost::transition`;
5. refresh shell status/conflict chrome named by `SessionChange` or the
   outcome.

It must not inspect a concrete view type or mutate `OpenTabs` directly.

Conflict deletion becomes a shell-produced operation passed through the same
session mutation helper as a view-produced placement operation. The conflict
popup's keep-open/re-anchor behavior remains shell popup policy and is preserved
after the shared transaction completes.

## Unit 5 — explicit action coordination

Move action routing into `crates/waml-editor/src/app/actions.rs`, implemented as
inherent methods on `App`. The `MatchEvent::handle_actions` trait method remains
in `app.rs` as a one-line forwarder to `handle_action_batch`; the ordered
coordinator itself lives in the new module.

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ActionFlow {
    Continue,
    Consumed,
}
```

The coordinator has two explicit phases.

### Phase A: non-exclusive observers

These handlers always complete in this order and then allow later handlers to
inspect the same `Actions` batch:

1. caption burger press and dock toggles;
2. popup close/armed results;
3. conflict-list focus/delete effects.

This preserves current behavior: these branches do not return from
`handle_actions`, even when they act.

### Phase B: exclusive priority chain

Each handler returns `Consumed` or `Continue`. The first consumed action ends
the chain:

1. navigation-scope popup request;
2. navigation query change;
3. navigation-filter popup request;
4. tree context menu;
5. tree document open;
6. diagram switcher;
7. conflict badge;
8. active `DocView`;
9. logo menu request;
10. start-screen action;
11. shortcuts overlay dismissal;
12. fonts overlay dismissal;
13. icons overlay dismissal;
14. colors overlay dismissal;
15. document-tab-strip action.

This order is copied from the current method and is a compatibility contract.
The refactor may group implementation details but may not reorder handlers.

Popup presentation remains centralized because it requires window bounds and
the one `popup_root`. Popup results that belong to a document view are routed to
the active registered view; global popup results remain shell handlers.

## Unit 6 — shell projections and persistence adapter

`App` retains:

- root `ui` and Makepad lifecycle;
- start screen, recents, overlays, theme, keyboard/tool state;
- project-tree navigation state and popup id maps;
- dock responsive policy;
- popup placement;
- save timer and platform save backend;
- statusbar and global conflict badge projection.

These responsibilities are shell-global and should not migrate into
`EditorSession` or `DocView`.

Model and bundle reads change from `self.model` / `self.bundle` to
`self.session.model()` / `self.session.bundle()`. Mutable tab access changes to
`DocumentHost` commands. This makes bypasses visible to the compiler.

On browser save, `App` marks the exact saved session revision clean after
updating the URL. On native, the existing no-op backend does not mark the
revision clean. There is no new dirty indicator or save-error UI in this
behavior-preserving change.

## Error handling

- `EditorSession::apply_ops` returns the existing `waml::ops::OpError`.
- A failed operation logs through the current shell mechanism, changes no
  session state, triggers no view invalidation, and schedules no save.
- Missing view source continues to render the current italic fallback.
- Missing model keys continue to make document-open requests no-ops.
- A missing active tab hides all view-owned chrome and skips view dispatch.
- Registry reconciliation treats `OpenTabs` as authoritative: closed and
  replaced tab ids cannot retain live view objects.

No new user-facing error UI is introduced.

## Testing

### Characterization before production changes

- Capture the existing `OpenTabs` behavior for preview replacement, promotion,
  activation fallback after close, source-tab identity, and diagram switching.
- Add pure tests that freeze the two action phases and the exclusive handler
  priority listed above.
- Preserve existing popup-result ordering, especially placement-dial armed
  before closed in one action batch.

### `EditorSession`

- successful ops replace bundle/model once, increment revision once, and mark
  the new revision dirty;
- failed ops leave all state and revision unchanged;
- `replace` performs full invalidation and starts clean;
- `mark_saved(old_revision)` cannot clear a newer dirty revision;
- both `PlaceSet` and `PlaceRm` travel through the same transaction.

### `DocumentHost` and views

- every tab transition reconciles the registry;
- preview replacement drops the replaced live view;
- closing the active tab activates and synchronizes the same fallback tab as
  today;
- chrome and accent come from the live view without constructing another view;
- source synchronization reads the raw bundle from `ViewData`;
- a diagram's post-mutation refresh uses the camera-preserving path;
- no downcast API remains.

### Verification

- `cargo fmt --check`
- focused `waml-editor` tests after every stage;
- `cargo test --workspace`
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- native visual parity screenshots of the start screen, class diagram,
  classifier preview, source view, tab switching, popups, overlays, and both
  dock states. Capture the baseline before implementation and compare the same
  scenarios afterward at identical window size and scale.

## Migration sequence

1. Add characterization tests and capture the visual baseline.
2. Introduce `EditorSession`; route both mutation paths through it.
3. Make concrete views self-identifying and session-change aware; remove
   downcasts and the source special case.
4. Introduce `DocumentHost`; route all tab transitions through it.
5. Query chrome/accent from live views and remove temporary-view helpers.
6. Extract the ordered action handlers without changing their order.
7. Remove unused outcome scaffolding and obsolete shell helpers.
8. Run full automated and visual parity verification.

Each stage must compile and pass its focused tests before the next stage begins.

## Non-goals

- Native durable persistence.
- Undo/redo history.
- Incremental parsing or partial model rebuild.
- New document view kinds.
- Dynamic Makepad widget-subtree mounting.
- Per-tab right-dock open/closed state.
- UI redesign or cleanup discovered while moving code.
- Canvas/controller/rendering decomposition; that remains a separate issue.

## Success criteria

- `App` no longer owns bundle, model, mutable tabs, or the view registry as
  independent fields.
- There is one operation-application transaction and one tab-transition
  choreography.
- `handle_actions` visibly declares the existing priority and delegates each
  cohesive concern.
- `App` contains no `TabKind::Source` synchronization branch, concrete
  `DocView` downcast, or concrete post-mutation refresh.
- chrome and accent are read from the registered live view.
- unused outcome channels are absent.
- all automated checks pass and the UI is visually and behaviorally unchanged.
