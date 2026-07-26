# Editor Architecture Ownership Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Transfer document data, tab/view lifecycle, and document-local behavior out of `App` while preserving the editor's current UI, interactions, action priority, and save timing exactly.

**Architecture:** `EditorSession` becomes the framework-light owner of bundle/model/revision/dirty state and the sole operation transaction. `DocumentHost` owns `OpenTabs`, the live `Box<dyn DocView>` registry, every tab transition, and synchronization of the one shared Makepad body surface; self-identifying views read `ViewData` and emit typed `ViewOutcome`s. `App` remains the shell composition root and runs an explicit two-phase action coordinator, popup/platform policy, global projections, and persistence scheduling.

**Tech Stack:** Rust 2021 (workspace MSRV), Makepad widgets/script DSL, WAML model/parser/operations, inline Rust unit tests in the binary-only `waml-editor` crate, PowerShell/PrintWindow native visual verification.

## Global Constraints

- No intentional visual change.
- No interaction, shortcut, popup, overlay, dock, tab, startup, or save-timing change.
- Preserve the current event priority, including actions that intentionally continue through the batch rather than consuming it.
- Preserve one shared Makepad body widget surface; do not mount a widget subtree per tab.
- Preserve `Box<dyn DocView>` as the extension seam.
- Only the composition-root factory in `document_host.rs` may match `TabKind` to a concrete view constructor.
- Add no dependencies and do not change `crates/waml-editor/Cargo.toml`.
- Keep native persistence a no-op and do not mark a native revision saved.
- Do not add dirty, save-error, or other user-facing UI.
- `EditorSession` must know nothing about `Cx`, widgets, tabs, popups, or persistence backends.
- `DocumentHost` must not own project-tree navigation, recents, overlays, popup placement, or persistence.
- `DocView` may read session data and emit document-local intent; it must not mutate the session, tabs, or popup root.
- Keep right-dock open/closed state app-global; a view declares only its availability and glyph.
- Keep tests inline under `#[cfg(test)]`; `waml-editor` is binary-only and has no `--lib` target.
- Every shell command in this plan must be prefixed with `rtk`, per `RTK.md`.
- Each task must compile and pass its focused tests before the next task begins.

---

## File Structure

The completed ownership layout is:

```text
crates/waml-editor/src/
├── main.rs                         # declares editor_session + document_host
├── editor_session.rs               # bundle/model/revision/dirty transaction
├── document_host.rs                # OpenTabs + live views + transition choreography
├── doc_tabs.rs                     # pure tab state and tab-strip widget
├── doc_view.rs                     # BodyWidgets, ViewData, DocView, outcomes/chrome
├── class_diagram_view.rs           # diagram identity, sync, actions, camera-held refresh
├── classifier_preview_view.rs      # classifier identity, sync, actions, accent
├── source_view.rs                  # source identity and raw-bundle synchronization
└── app/
    └── actions.rs                  # explicit action phases, handlers, outcome application
```

`app.rs` remains the Makepad shell, lifecycle owner, popup presenter, navigation owner, persistence adapter, and home of the existing script DSL. Do not split unrelated layout, dock, popup-placement, or startup helpers out of it during this refactor.

The cross-unit interfaces are fixed by the approved design:

```rust
// editor_session.rs
#[derive(Default)]
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

// doc_view.rs
#[derive(Clone, Copy)]
pub struct ViewData<'a> {
    pub model: &'a waml::model::Model,
    pub bundle: &'a [(String, String)],
    pub revision: u64,
}

// document_host.rs
pub struct DocumentHost {
    tabs: OpenTabs,
    views: HashMap<LiveId, Box<dyn DocView>>,
}
```

`DocumentHost` is the only mutable-tab façade. Its complete public API after Task 4 is:

```rust
impl DocumentHost {
    pub fn active_tab(&self) -> Option<&DocTab>;
    pub fn tabs(&self) -> &[DocTab];
    pub fn active_id(&self) -> LiveId;
    pub fn active_chrome(&self) -> BodyChrome;
    pub fn active_accent(&self) -> Option<Vec4>;

    pub fn transition(
        &mut self,
        cx: &mut Cx,
        ui: &WidgetRef,
        session: &EditorSession,
        command: DocumentCommand,
    ) -> bool;

    pub fn replace_for_session(
        &mut self,
        cx: &mut Cx,
        ui: &WidgetRef,
        session: &EditorSession,
        tabs: OpenTabs,
    ) -> bool;

    pub fn sync_active(&mut self, cx: &mut Cx, ui: &WidgetRef, session: &EditorSession);
    pub fn after_session_change(
        &mut self,
        cx: &mut Cx,
        ui: &WidgetRef,
        session: &EditorSession,
        change: SessionChange,
    );
    pub fn handle_active(
        &mut self,
        cx: &mut Cx,
        ui: &WidgetRef,
        actions: &Actions,
        session: &EditorSession,
    ) -> Option<ViewOutcome>;
    pub fn on_active_popup_result(
        &mut self,
        cx: &mut Cx,
        ui: &WidgetRef,
        session: &EditorSession,
        tag: LiveId,
        result: PopupResult,
    ) -> Option<ViewOutcome>;
    pub fn on_active_popup_armed(
        &mut self,
        cx: &mut Cx,
        ui: &WidgetRef,
        session: &EditorSession,
        tag: LiveId,
        id: Option<LiveId>,
    ) -> Option<ViewOutcome>;
}
```

`sync_active` is public only for shell rehydration, where no tab mutation occurs. User tab changes and complete session replacement must use `transition` and `replace_for_session`.

### Task 1: Freeze Action and Tab Behavior, Then Capture the Native Baseline

**Files:**
- Create: `crates/waml-editor/src/app/actions.rs`
- Modify: `crates/waml-editor/src/app.rs:1-20,1975-2710,3143-3348`
- Modify: `crates/waml-editor/src/doc_tabs.rs:1097-1267`
- Test: inline tests in `crates/waml-editor/src/app/actions.rs`
- Test: inline tests in `crates/waml-editor/src/doc_tabs.rs`

**Interfaces:**
- Consumes: the existing source-order contract in `App::handle_actions`, existing `OpenTabs` mutation behavior, and `PopupRoot` emitting `Armed` and `Closed` in one action batch.
- Produces: `ObserverHandler`, `ExclusiveHandler`, `PopupRelay`, `ActionFlow`, and the three exact order constants later consumed by the production coordinator.

- [ ] **Step 1: Run the pre-refactor automated baseline**

Run:

```powershell
rtk cargo test -p waml-editor
```

Expected: PASS for the binary unit-test harness. Record the test count in the execution notes and stop on any unexplained failure.

- [ ] **Step 2: Add the pure action-order contract**

Add `mod actions;` immediately after the imports/module declarations at the top of `app.rs`. Create `app/actions.rs` with these exact types and constants:

```rust
use super::App;
use makepad_widgets::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ActionFlow {
    Continue,
    Consumed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ObserverHandler {
    CaptionAndDocks,
    PopupResults,
    ConflictList,
}

const OBSERVER_ORDER: [ObserverHandler; 3] = [
    ObserverHandler::CaptionAndDocks,
    ObserverHandler::PopupResults,
    ObserverHandler::ConflictList,
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PopupRelay {
    ElementPickerClosed,
    PlaceDialArmed,
    PlaceDialClosed,
}

const DOCUMENT_POPUP_RELAY_ORDER: [PopupRelay; 3] = [
    PopupRelay::ElementPickerClosed,
    PopupRelay::PlaceDialArmed,
    PopupRelay::PlaceDialClosed,
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ExclusiveHandler {
    NavigationScope,
    NavigationQuery,
    NavigationFilter,
    TreeContextMenu,
    TreeDocumentOpen,
    DiagramSwitcher,
    ConflictBadge,
    ActiveDocumentView,
    LogoMenu,
    StartScreen,
    ShortcutsOverlay,
    FontsOverlay,
    IconsOverlay,
    ColorsOverlay,
    DocumentTabs,
}

const EXCLUSIVE_ORDER: [ExclusiveHandler; 15] = [
    ExclusiveHandler::NavigationScope,
    ExclusiveHandler::NavigationQuery,
    ExclusiveHandler::NavigationFilter,
    ExclusiveHandler::TreeContextMenu,
    ExclusiveHandler::TreeDocumentOpen,
    ExclusiveHandler::DiagramSwitcher,
    ExclusiveHandler::ConflictBadge,
    ExclusiveHandler::ActiveDocumentView,
    ExclusiveHandler::LogoMenu,
    ExclusiveHandler::StartScreen,
    ExclusiveHandler::ShortcutsOverlay,
    ExclusiveHandler::FontsOverlay,
    ExclusiveHandler::IconsOverlay,
    ExclusiveHandler::ColorsOverlay,
    ExclusiveHandler::DocumentTabs,
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn non_exclusive_observers_keep_the_existing_order() {
        assert_eq!(
            OBSERVER_ORDER,
            [
                ObserverHandler::CaptionAndDocks,
                ObserverHandler::PopupResults,
                ObserverHandler::ConflictList,
            ]
        );
    }

    #[test]
    fn exclusive_handlers_keep_the_existing_priority() {
        assert_eq!(
            EXCLUSIVE_ORDER,
            [
                ExclusiveHandler::NavigationScope,
                ExclusiveHandler::NavigationQuery,
                ExclusiveHandler::NavigationFilter,
                ExclusiveHandler::TreeContextMenu,
                ExclusiveHandler::TreeDocumentOpen,
                ExclusiveHandler::DiagramSwitcher,
                ExclusiveHandler::ConflictBadge,
                ExclusiveHandler::ActiveDocumentView,
                ExclusiveHandler::LogoMenu,
                ExclusiveHandler::StartScreen,
                ExclusiveHandler::ShortcutsOverlay,
                ExclusiveHandler::FontsOverlay,
                ExclusiveHandler::IconsOverlay,
                ExclusiveHandler::ColorsOverlay,
                ExclusiveHandler::DocumentTabs,
            ]
        );
    }

    #[test]
    fn placement_dial_armed_is_relayed_before_closed() {
        let armed = DOCUMENT_POPUP_RELAY_ORDER
            .iter()
            .position(|handler| *handler == PopupRelay::PlaceDialArmed)
            .unwrap();
        let closed = DOCUMENT_POPUP_RELAY_ORDER
            .iter()
            .position(|handler| *handler == PopupRelay::PlaceDialClosed)
            .unwrap();
        assert!(armed < closed);
    }
}
```

Until Task 6 wires the constants into production, put
`#![cfg_attr(not(test), allow(dead_code, unused_imports))]` at the first line of
`actions.rs`. Task 6 must remove that temporary allowance.

- [ ] **Step 3: Add the missing same-subject source identity characterization**

Add this test beside the existing source-tab tests in `doc_tabs.rs`:

```rust
#[test]
fn classifier_and_source_tabs_for_one_subject_have_distinct_stable_ids() {
    let classifier = classifier_tab_id("customer");
    let source = source_tab_id("customer");
    assert_ne!(classifier, source);

    let mut open = OpenTabs::default();
    let classifier_open =
        open.open_preview("customer", "Customer", TreeKind::Class);
    open.promote(classifier_open);
    let source_open = open.open_source("customer", "Customer");

    assert_eq!(classifier_open, classifier);
    assert_eq!(source_open, source);
    assert_eq!(open.tabs.len(), 2);
    assert_eq!(open.active, source);
}
```

Do not rewrite the existing preview replacement, promotion, close fallback,
source replacement, or `diagram_switcher::next_diagram_key` tests; those are
the required characterizations already present.

- [ ] **Step 4: Run the characterization tests**

Run:

```powershell
rtk cargo test -p waml-editor app::actions::tests
rtk cargo test -p waml-editor doc_tabs::tests
rtk cargo test -p waml-editor diagram_switcher::tests
```

Expected: all three commands PASS. The action-order tests must run three tests,
and the existing tab/switcher behavior must remain green.

- [ ] **Step 5: Capture native UI baselines at a fixed window size and DPI**

Run the following from the worktree root in an interactive PowerShell terminal.
It captures the start screen and then pauses for seven editor states. Keep the
Windows display scale unchanged until Task 8. The helper forces the same outer
window rectangle before every capture; `capture-window.ps1` records the native
client pixels.

```powershell
rtk proxy pwsh -NoProfile -Command @'
$ErrorActionPreference = "Stop"
$editorOwnershipRoot = (Resolve-Path ".").Path
$editorOwnershipTarget = Join-Path $editorOwnershipRoot "target"
$editorOwnershipExe = Join-Path $editorOwnershipTarget "debug\waml-editor.exe"
$editorOwnershipCapture = Join-Path $editorOwnershipRoot "scripts\capture-window.ps1"
$editorOwnershipOut = "C:\tmp\editor-ownership-before"
New-Item -ItemType Directory -Force -Path $editorOwnershipOut | Out-Null

Add-Type @"
using System;
using System.Runtime.InteropServices;
public static class EditorOwnershipWindow {
    [DllImport("user32.dll", SetLastError=true)]
    public static extern bool MoveWindow(
        IntPtr hWnd, int x, int y, int width, int height, bool repaint);
}
"@

& rtk cargo build -p waml-editor --target-dir $editorOwnershipTarget
if ($LASTEXITCODE -ne 0) { throw "waml-editor build failed" }

function Start-EditorOwnershipWindow {
    param([string[]]$EditorOwnershipArgs)
    $editorOwnershipProcess = Start-Process `
        -FilePath $editorOwnershipExe `
        -ArgumentList $EditorOwnershipArgs `
        -WorkingDirectory $editorOwnershipRoot `
        -WindowStyle Normal `
        -PassThru
    $editorOwnershipDeadline = (Get-Date).AddSeconds(30)
    do {
        Start-Sleep -Milliseconds 200
        $editorOwnershipProcess.Refresh()
        if ($editorOwnershipProcess.HasExited) {
            throw "editor pid=$($editorOwnershipProcess.Id) exited before opening a window"
        }
    } while (
        $editorOwnershipProcess.MainWindowHandle -eq 0 -and
        (Get-Date) -lt $editorOwnershipDeadline
    )
    if ($editorOwnershipProcess.MainWindowHandle -eq 0) {
        throw "editor pid=$($editorOwnershipProcess.Id) opened no window"
    }
    [EditorOwnershipWindow]::MoveWindow(
        $editorOwnershipProcess.MainWindowHandle, 40, 40, 1280, 900, $true) | Out-Null
    Start-Sleep -Milliseconds 500
    return $editorOwnershipProcess
}

function Save-EditorOwnershipShot {
    param(
        [System.Diagnostics.Process]$EditorOwnershipProcess,
        [string]$EditorOwnershipName
    )
    & rtk pwsh -File $editorOwnershipCapture `
        -Out (Join-Path $editorOwnershipOut "$EditorOwnershipName.png") `
        -ProcessId $editorOwnershipProcess.Id
    if ($LASTEXITCODE -ne 0) { throw "capture failed: $EditorOwnershipName" }
}

$editorOwnershipStart = Start-EditorOwnershipWindow -EditorOwnershipArgs @()
try {
    $null = Read-Host "Confirm the empty start screen is settled; press Enter"
    Save-EditorOwnershipShot $editorOwnershipStart "start-screen"
}
finally {
    Stop-Process -Id $editorOwnershipStart.Id -ErrorAction SilentlyContinue
}

$editorOwnershipFixture =
    (Resolve-Path "crates/waml-editor/tests/fixtures/mini").Path
$editorOwnershipEditor =
    Start-EditorOwnershipWindow -EditorOwnershipArgs @($editorOwnershipFixture)
try {
    $editorOwnershipPrompts = @(
        @("class-diagram", "Show Orders diagram with both docks open"),
        @("classifier-preview", "Open Customer as the active classifier preview"),
        @("source-view", "Open Customer context menu and choose View Source"),
        @("tab-switching", "Pin Orders and Customer, then activate Orders from the tab strip"),
        @("popup", "Open the burger menu without moving or resizing the window"),
        @("overlay", "Open the shortcuts overlay"),
        @("docks-closed", "Return to Orders and close both left and right docks")
    )
    foreach ($editorOwnershipEntry in $editorOwnershipPrompts) {
        $null = Read-Host "$($editorOwnershipEntry[1]); press Enter"
        Save-EditorOwnershipShot $editorOwnershipEditor $editorOwnershipEntry[0]
    }
}
finally {
    Stop-Process -Id $editorOwnershipEditor.Id -ErrorAction SilentlyContinue
}
'@
```

Expected: eight native-resolution PNGs under
`C:\tmp\editor-ownership-before`. These are local verification artifacts and
must not be staged or committed.

- [ ] **Step 6: Commit the characterization boundary**

```powershell
rtk git add crates/waml-editor/src/app.rs crates/waml-editor/src/app/actions.rs crates/waml-editor/src/doc_tabs.rs
rtk git commit -m "test(editor): freeze ownership refactor behavior"
```

### Task 2: Introduce `EditorSession` and Route Both Mutation Producers Through It

**Files:**
- Create: `crates/waml-editor/src/editor_session.rs`
- Modify: `crates/waml-editor/src/main.rs:1-45`
- Modify: `crates/waml-editor/src/app.rs:540-563,657-779,1264-1306,1352-1477,1551-1560,1649-1680,1975-2888`
- Test: inline tests in `crates/waml-editor/src/editor_session.rs`
- Test: existing conflict operation tests in `crates/waml-editor/src/app.rs`

**Interfaces:**
- Consumes: `waml::ops::apply(&[(String, String)], &[Op]) -> Result<Vec<(String, String)>, OpError>` and `waml::parse::build_model(&[(String, String)]) -> Model`.
- Produces: `EditorSession::{replace,apply_ops,bundle,model,revision,is_dirty,mark_saved}` and `SessionChange` exactly as shown below. `App` has one `session: EditorSession` field in place of `model` and `bundle`.

- [ ] **Step 1: Write failing session transaction tests**

Create `editor_session.rs` with imports, test helpers, and the tests first:

```rust
use waml::model::Model;
use waml::ops::{Op, OpError};

#[cfg(test)]
mod tests {
    use super::*;
    use waml::syntax::Direction;

    fn diagram_bundle(layout: &str) -> Vec<(String, String)> {
        vec![(
            "dia.md".to_string(),
            format!(
                "---\ntype: Diagram\ntitle: D\nprofile: uml-domain\n---\n# D\n\n## Layout\n{layout}"
            ),
        )]
    }

    fn place_set() -> Op {
        Op::PlaceSet {
            diagram: "dia".into(),
            subject_title: "Order".into(),
            subject_slug: "order".into(),
            reference_title: "Customer".into(),
            reference_slug: "customer".into(),
            directions: vec![Direction::LeftOf],
        }
    }

    fn place_rm() -> Op {
        Op::PlaceRm {
            diagram: "dia".into(),
            subject_slug: "order".into(),
            reference_slug: "customer".into(),
        }
    }

    #[test]
    fn replace_fully_invalidates_and_starts_clean() {
        let bundle = diagram_bundle("");
        let model = waml::parse::build_model(&bundle);
        let mut session = EditorSession::default();

        let change = session.replace(bundle.clone(), model);

        assert_eq!(change, SessionChange::full(1));
        assert_eq!(session.bundle(), bundle.as_slice());
        assert_eq!(session.revision(), 1);
        assert!(!session.is_dirty());
    }

    #[test]
    fn successful_ops_increment_once_and_mark_the_revision_dirty() {
        let bundle = diagram_bundle("");
        let model = waml::parse::build_model(&bundle);
        let mut session = EditorSession::default();
        session.replace(bundle, model);

        let change = session.apply_ops(&[place_set()]).unwrap();

        assert_eq!(change, SessionChange::full(2));
        assert_eq!(session.revision(), 2);
        assert!(session.is_dirty());
        assert!(session.bundle()[0].1.contains("left of"));
    }

    #[test]
    fn failed_ops_leave_bundle_model_revision_and_dirty_state_unchanged() {
        let bundle = diagram_bundle("");
        let model = waml::parse::build_model(&bundle);
        let mut session = EditorSession::default();
        session.replace(bundle, model);
        let before_bundle = session.bundle().to_vec();
        let before_model = session.model().clone();
        let before_revision = session.revision();

        let result = session.apply_ops(&[Op::AttrRm {
            node: "missing".into(),
            name: "also-missing".into(),
        }]);

        assert!(result.is_err());
        assert_eq!(session.bundle(), before_bundle.as_slice());
        assert_eq!(session.model(), &before_model);
        assert_eq!(session.revision(), before_revision);
        assert!(!session.is_dirty());
    }

    #[test]
    fn saving_an_old_revision_cannot_clear_a_newer_dirty_revision() {
        let bundle = diagram_bundle("");
        let model = waml::parse::build_model(&bundle);
        let mut session = EditorSession::default();
        session.replace(bundle, model);
        let old = session.revision();
        session.apply_ops(&[place_set()]).unwrap();

        session.mark_saved(old);
        assert!(session.is_dirty());

        session.mark_saved(session.revision());
        assert!(!session.is_dirty());
    }

    #[test]
    fn place_set_and_place_rm_use_the_same_transaction() {
        let bundle = diagram_bundle("");
        let model = waml::parse::build_model(&bundle);
        let mut session = EditorSession::default();
        session.replace(bundle, model);

        let set = session.apply_ops(&[place_set()]).unwrap();
        assert!(session.bundle()[0].1.contains("left of"));
        let remove = session.apply_ops(&[place_rm()]).unwrap();

        assert_eq!(set.revision + 1, remove.revision);
        assert!(!session.bundle()[0].1.contains("left of"));
        assert!(session.is_dirty());
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run:

```powershell
rtk cargo test -p waml-editor editor_session::tests
```

Expected: FAIL because `main.rs` does not declare `editor_session` and
`EditorSession`/`SessionChange` are not defined.

- [ ] **Step 3: Implement the atomic session**

Add `mod editor_session;` in `main.rs`, then add this implementation above the
tests in `editor_session.rs`:

```rust
#[derive(Default)]
pub struct EditorSession {
    bundle: Vec<(String, String)>,
    model: Model,
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

impl SessionChange {
    fn full(revision: u64) -> SessionChange {
        SessionChange {
            revision,
            model_changed: true,
            source_changed: true,
            navigation_changed: true,
            conflicts_changed: true,
        }
    }
}

impl EditorSession {
    pub fn replace(
        &mut self,
        bundle: Vec<(String, String)>,
        model: Model,
    ) -> SessionChange {
        self.bundle = bundle;
        self.model = model;
        self.revision = self.revision.wrapping_add(1);
        self.dirty_revision = None;
        SessionChange::full(self.revision)
    }

    pub fn apply_ops(&mut self, ops: &[Op]) -> Result<SessionChange, OpError> {
        let bundle = waml::ops::apply(&self.bundle, ops)?;
        let model = waml::parse::build_model(&bundle);
        self.bundle = bundle;
        self.model = model;
        self.revision = self.revision.wrapping_add(1);
        self.dirty_revision = Some(self.revision);
        Ok(SessionChange::full(self.revision))
    }

    pub fn bundle(&self) -> &[(String, String)] {
        &self.bundle
    }

    pub fn model(&self) -> &Model {
        &self.model
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty_revision.is_some()
    }

    pub fn mark_saved(&mut self, revision: u64) {
        if self.dirty_revision == Some(revision) {
            self.dirty_revision = None;
        }
    }
}
```

Use `wrapping_add` so the method is total in debug and release builds. One
successful `apply_ops` call invokes `waml::ops::apply` once, builds one `Model`,
and commits all four fields only after both values exist.

- [ ] **Step 4: Run the session tests**

Run:

```powershell
rtk cargo test -p waml-editor editor_session::tests
```

Expected: PASS for all five session tests.

- [ ] **Step 5: Replace `App`'s independent document fields**

Replace:

```rust
#[rust]
model: Model,
#[rust]
bundle: Vec<(String, String)>,
```

with:

```rust
#[rust]
session: crate::editor_session::EditorSession,
```

Mechanically change every read:

```rust
self.model              -> self.session.model()
&self.model             -> self.session.model()
self.bundle             -> self.session.bundle()
&self.bundle            -> self.session.bundle()
```

Resolve resulting borrow expressions deliberately; do not use an automated
replacement on assignment sites. In `open_bundle`, replace the two assignments
with:

```rust
let change = self.session.replace(bundle, model);
debug_assert_eq!(change.revision, self.session.revision());
```

All later navigation, title, tree, source, diagram, and switcher reads in that
method must come from `self.session.model()`.

- [ ] **Step 6: Route both existing operation producers through `EditorSession`**

In the conflict-delete branch, replace the direct `waml::ops::apply`/bundle/model
assignment with:

```rust
match self.session.apply_ops(&[op]) {
    Ok(_change) => {
        if let Some(active) = self.tabs.active_tab().cloned() {
            if let Some(v) = self
                .views
                .get_mut(&active.id)
                .and_then(|v| v.downcast_diagram())
            {
                v.resolve_active(cx, &body, self.session.model());
            }
        }
        self.sync_conflict_badge(cx);
        self.mark_dirty(cx);

        let conflicts = self
            .ui
            .widget(cx, ids!(canvas))
            .borrow::<crate::canvas::ClassDiagramSurface>()
            .map(|c| c.conflicts())
            .unwrap_or_default();
        if conflicts.is_empty() {
            if let Some(mut pr) = self
                .ui
                .widget(cx, ids!(popup_root))
                .borrow_mut::<PopupRoot>()
            {
                pr.close(cx);
            }
        } else {
            self.open_conflict_list(cx, conflicts);
        }
    }
    Err(e) => log!("place.rm failed: {e:?}"),
}
```

In `relay_outcome`, make the equivalent replacement:

```rust
if !outcome.ops.is_empty() {
    match self.session.apply_ops(&outcome.ops) {
        Ok(_change) => {
            let body = crate::doc_view::BodyWidgets::new(cx, &self.ui);
            if let Some(v) = self
                .views
                .get_mut(&active.id)
                .and_then(|v| v.downcast_diagram())
            {
                v.resolve_active(cx, &body, self.session.model());
            }
            self.sync_conflict_badge(cx);
            self.mark_dirty(cx);
        }
        Err(e) => log!("place.set failed: {e:?}"),
    }
}
```

This task intentionally retains the downcast and duplicated post-transaction
refresh; Task 3 removes the concrete-view dependency and Task 5 unifies the
success tail.

- [ ] **Step 7: Make save completion revision-aware without changing timing**

Keep `mark_dirty` and `SAVE_DEBOUNCE_SECS` unchanged. Replace `save` and the two
backends with:

```rust
fn save(&mut self, cx: &mut Cx) {
    if self.session.bundle().is_empty() {
        return;
    }
    let revision = self.session.revision();
    if self.save_backend(cx) {
        self.session.mark_saved(revision);
    }
}

#[cfg(target_arch = "wasm32")]
fn save_backend(&mut self, cx: &mut Cx) -> bool {
    cx.browser_update_url(
        &format!("#{}", waml::share::encode(self.session.bundle())),
        true,
    );
    true
}

#[cfg(not(target_arch = "wasm32"))]
fn save_backend(&mut self, _cx: &mut Cx) -> bool {
    false
}
```

The browser marks exactly the encoded revision clean. The native no-op reports
`false`, so dirty state remains accurate.

- [ ] **Step 8: Verify the session migration**

Run:

```powershell
rtk cargo fmt --check
rtk cargo test -p waml-editor editor_session::tests
rtk cargo test -p waml-editor app::tests::conflict_delete
rtk cargo test -p waml-editor
```

Expected: all commands PASS. Search for bypasses:

```powershell
rtk rg -n "model: Model|bundle: Vec<|waml::ops::apply" crates/waml-editor/src/app.rs
```

Expected: no App document fields and no direct operation application.

- [ ] **Step 9: Commit the session ownership transfer**

```powershell
rtk git add crates/waml-editor/src/main.rs crates/waml-editor/src/editor_session.rs crates/waml-editor/src/app.rs
rtk git commit -m "refactor(editor): centralize document session"
```

### Task 3: Make Every Live `DocView` Self-Identifying and Session-Aware

**Files:**
- Modify: `crates/waml-editor/src/doc_view.rs:13-297,299-478`
- Modify: `crates/waml-editor/src/class_diagram_view.rs:4-188,474-567`
- Modify: `crates/waml-editor/src/classifier_preview_view.rs:4-143`
- Modify: `crates/waml-editor/src/source_view.rs:1-90`
- Modify: `crates/waml-editor/src/app.rs:641-731,2166-2205,2247-2263,2522-2533,2717-2754`
- Test: inline tests in `crates/waml-editor/src/doc_view.rs`
- Test: inline tests in `crates/waml-editor/src/class_diagram_view.rs`
- Test: inline tests in `crates/waml-editor/src/source_view.rs`

**Interfaces:**
- Consumes: `EditorSession::{model,bundle,revision}`, `SessionChange`, the single shared `BodyWidgets` surface, and immutable identity from `DocTab`.
- Produces: the approved `ViewData`/`DocView` contract, `BodyChrome::HIDDEN`, self-identifying constructors, source-owned markdown synchronization, and camera-preserving `ClassDiagramView::after_session_change`.

- [ ] **Step 1: Write failing self-identifying-view contract tests**

Add this test to `class_diagram_view.rs`:

```rust
#[test]
fn diagram_view_is_constructed_with_immutable_identity() {
    use super::ClassDiagramView;
    use crate::doc_view::DocView;

    let view = ClassDiagramView::new("orders".into(), "Orders".into());

    assert_eq!(
        view.chrome(),
        crate::doc_view::BodyChrome {
            tool_dock: true,
            view_bar: true,
            right_dock: Some(crate::icons::Icon::SlidersHorizontal),
        }
    );
}
```

Add this test to `source_view.rs`:

```rust
#[cfg(test)]
mod ownership_contract_tests {
    use super::*;
    use crate::doc_view::DocView;
    use crate::tree::TreeKind;

    #[test]
    fn source_view_is_constructed_with_all_tab_identity() {
        let view = SourceView::new("shop/order".into(), TreeKind::Enum);

        assert_eq!(
            view.tab_accent(),
            Some(crate::accent::bucket_color(
                crate::node_style::AccentBucket::None,
            ))
        );
    }
}
```

- [ ] **Step 2: Run the ownership contract tests to verify they fail**

```powershell
rtk cargo test -p waml-editor diagram_view_is_constructed_with_immutable_identity
rtk cargo test -p waml-editor source_view_is_constructed_with_all_tab_identity
```

Expected: both commands fail to compile because the current constructors do
not accept complete tab identity and the current `DocView` contract does not
provide `chrome()` or a self-contained `tab_accent()`.

- [ ] **Step 3: Replace the trait contract and add view-data/chrome tests**

Replace the current `DocView` trait, downcast helper, throwaway
`body_chrome`/`tab_accent` helpers, and right-dock request helper with:

```rust
#[derive(Clone, Copy)]
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

    fn on_popup_result(
        &mut self,
        cx: &mut Cx,
        body: &BodyWidgets,
        data: ViewData<'_>,
        tag: LiveId,
        result: PopupResult,
    ) -> ViewOutcome {
        let _ = (cx, body, data, tag, result);
        ViewOutcome::default()
    }

    fn on_popup_armed(
        &mut self,
        cx: &mut Cx,
        body: &BodyWidgets,
        data: ViewData<'_>,
        tag: LiveId,
        id: Option<LiveId>,
    ) -> ViewOutcome {
        let _ = (cx, body, data, tag, id);
        ViewOutcome::default()
    }

    fn after_session_change(
        &mut self,
        cx: &mut Cx,
        body: &BodyWidgets,
        data: ViewData<'_>,
        _change: SessionChange,
    ) {
        self.sync(cx, body, data);
    }

    fn chrome(&self) -> BodyChrome;

    fn tab_accent(&self) -> Option<Vec4> {
        None
    }

    fn on_activate(&mut self, cx: &mut Cx, body: &BodyWidgets) {
        let _ = (cx, body);
    }

    fn on_deactivate(&mut self, cx: &mut Cx, body: &BodyWidgets) {
        let _ = (cx, body);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BodyChrome {
    pub tool_dock: bool,
    pub view_bar: bool,
    pub right_dock: Option<Icon>,
}

impl BodyChrome {
    pub const HIDDEN: BodyChrome = BodyChrome {
        tool_dock: false,
        view_bar: false,
        right_dock: None,
    };
}
```

Add imports for `SessionChange`, remove `DocTab`/`TabKind` imports, and delete
`as_any_mut`, `downcast_diagram`, `body_chrome`, `tab_accent`,
`right_dock_open_requested`, and their tests.

Also import the custom icon-button extension used by the moved chrome code:

```rust
use crate::icon_button::IconButtonWidgetRefExt;
```

- [ ] **Step 4: Give `BodyWidgets` complete shared-surface/chrome operations**

Add these methods to `BodyWidgets`:

```rust
pub fn show_canvas(&self, cx: &mut Cx) {
    self.source_view(cx).set_visible(cx, false);
    self.ui
        .widget(cx, ids!(canvas_wrap))
        .set_visible(cx, true);
}

pub fn show_source(&self, cx: &mut Cx) {
    self.source_view(cx).set_visible(cx, true);
    self.ui
        .widget(cx, ids!(canvas_wrap))
        .set_visible(cx, false);
}

pub fn set_source_markdown(&self, cx: &mut Cx, markdown: &str) {
    self.ui
        .widget(cx, ids!(source_view.md))
        .as_markdown()
        .set_text(cx, markdown);
}

pub fn apply_chrome(&self, cx: &mut Cx, chrome: BodyChrome) {
    self.set_tool_dock_visible(cx, chrome.tool_dock);
    self.set_view_bar_visible(cx, chrome.view_bar);

    let button = self.ui.widget(cx, ids!(inspector_btn));
    if button.visible() != chrome.right_dock.is_some() {
        button.set_visible(cx, chrome.right_dock.is_some());
        cx.redraw_all();
    }
    if let Some(icon) = chrome.right_dock {
        button.as_icon_button().set_icon(cx, icon);
    }
    if let Some(mut tabs) = self
        .ui
        .widget(cx, ids!(doc_tabs))
        .borrow_mut::<crate::doc_tabs::DocTabs>()
    {
        tabs.set_right_dock_btn(cx, chrome.right_dock.is_some());
    }
    if chrome.right_dock.is_none() {
        if let Some(mut panel) = self
            .ui
            .widget(cx, ids!(inspector))
            .borrow_mut::<crate::inspector_panel::Inspector>()
        {
            panel.close_dock(cx);
        }
    }
}
```

This is a move of the current `App::sync_right_dock_btn` UI policy, not a
behavior change. `App` still owns and reads dock state in its responsive layout
code.

- [ ] **Step 5: Make `ClassDiagramView` own diagram identity and refresh policy**

Replace its derived default and mutable identity setter with:

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DiagramRefresh {
    PreserveCamera,
    None,
}

fn refresh_for(change: SessionChange) -> DiagramRefresh {
    if change.model_changed {
        DiagramRefresh::PreserveCamera
    } else {
        DiagramRefresh::None
    }
}

pub struct ClassDiagramView {
    key: String,
    title: String,
    expanded: HashSet<String>,
}

impl ClassDiagramView {
    pub fn new(key: String, title: String) -> ClassDiagramView {
        ClassDiagramView {
            key,
            title,
            expanded: HashSet::new(),
        }
    }

    fn update_scene(&self, cx: &mut Cx, body: &BodyWidgets, model: &Model) {
        if let Some(diagram) = model.diagrams.iter().find(|d| d.key == self.key) {
            let (scene, diagnostics) = build_scene(model, diagram, &self.expanded);
            for diagnostic in &diagnostics {
                log!("diagnostic: {diagnostic:?}");
            }
            if let Some(mut canvas) = body
                .canvas(cx)
                .borrow_mut::<crate::canvas::ClassDiagramSurface>()
            {
                canvas.update_scene(cx, scene);
            }
        }
    }
}
```

Rename every `active_key` use to `key`, every `active_title` use to `title`,
and change trait methods to accept `ViewData`. At each current `model` use, bind:

```rust
let model = data.model;
```

At the start of `sync`, add:

```rust
body.show_canvas(cx);
```

Add:

```rust
fn after_session_change(
    &mut self,
    cx: &mut Cx,
    body: &BodyWidgets,
    data: ViewData<'_>,
    change: SessionChange,
) {
    match refresh_for(change) {
        DiagramRefresh::PreserveCamera => {
            self.update_scene(cx, body, data.model);
        }
        DiagramRefresh::None => {}
    }
}

fn chrome(&self) -> BodyChrome {
    BodyChrome {
        tool_dock: true,
        view_bar: true,
        right_dock: Some(Icon::SlidersHorizontal),
    }
}
```

Delete public `set_active`, public `resolve_active`, and `as_any_mut`. The
`update_scene` method must call `ClassDiagramSurface::update_scene`, never
`set_scene`, so mutation retains the camera.

Add this test to `class_diagram_view.rs`:

```rust
#[test]
fn model_change_selects_the_camera_preserving_refresh() {
    assert_eq!(
        refresh_for(SessionChange {
            revision: 2,
            model_changed: true,
            source_changed: true,
            navigation_changed: true,
            conflicts_changed: true,
        }),
        DiagramRefresh::PreserveCamera
    );
    assert_eq!(
        refresh_for(SessionChange {
            revision: 3,
            model_changed: false,
            source_changed: true,
            navigation_changed: false,
            conflicts_changed: false,
        }),
        DiagramRefresh::None
    );
}
```

- [ ] **Step 6: Make classifier and source views own all immutable identity**

Change `ClassifierPreviewView` to:

```rust
pub struct ClassifierPreviewView {
    key: String,
    node_kind: TreeKind,
}

impl ClassifierPreviewView {
    pub fn new(key: String, node_kind: TreeKind) -> ClassifierPreviewView {
        ClassifierPreviewView { key, node_kind }
    }
}
```

At the start of its `sync`, call `body.show_canvas(cx)`. Change all trait
methods to `ViewData`, bind `let model = data.model`, and replace the old chrome
methods with:

```rust
fn chrome(&self) -> BodyChrome {
    BodyChrome {
        tool_dock: false,
        view_bar: false,
        right_dock: Some(Icon::SlidersHorizontal),
    }
}

fn tab_accent(&self) -> Option<Vec4> {
    crate::accent::tree_kind_color(self.node_kind)
}
```

Change `SourceView` to:

```rust
pub struct SourceView {
    key: String,
    node_kind: TreeKind,
}

impl SourceView {
    pub fn new(key: String, node_kind: TreeKind) -> SourceView {
        SourceView { key, node_kind }
    }

    fn markdown<'a>(&self, data: ViewData<'a>) -> std::borrow::Cow<'a, str> {
        crate::load::source_for(data.bundle, &self.key)
            .map(std::borrow::Cow::Borrowed)
            .unwrap_or_else(|| {
                std::borrow::Cow::Owned(format!("*No source for `{}`*", self.key))
            })
    }
}
```

Its `sync` starts with:

```rust
body.show_source(cx);
let markdown = self.markdown(data);
body.set_source_markdown(cx, markdown.as_ref());
let model = data.model;
```

Use this chrome/accent:

```rust
fn chrome(&self) -> BodyChrome {
    BodyChrome {
        tool_dock: false,
        view_bar: false,
        right_dock: Some(Icon::SlidersHorizontal),
    }
}

fn tab_accent(&self) -> Option<Vec4> {
    let _ = self.node_kind;
    Some(crate::accent::bucket_color(
        crate::node_style::AccentBucket::None,
    ))
}
```

Keeping `node_kind` on `SourceView` satisfies the uniform immutable-identity
contract even though the current source accent intentionally remains neutral.

- [ ] **Step 7: Add source and chrome unit tests**

Add to `source_view.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn data<'a>(
        model: &'a Model,
        bundle: &'a [(String, String)],
    ) -> ViewData<'a> {
        ViewData {
            model,
            bundle,
            revision: 7,
        }
    }

    #[test]
    fn source_markdown_reads_the_raw_bundle() {
        let model = Model::default();
        let bundle = vec![(
            "shop/order.md".to_string(),
            "# Order\nraw source".to_string(),
        )];
        let view = SourceView::new("shop/order".into(), TreeKind::Class);

        assert_eq!(view.markdown(data(&model, &bundle)), "# Order\nraw source");
    }

    #[test]
    fn missing_source_keeps_the_existing_italic_fallback() {
        let model = Model::default();
        let bundle = Vec::new();
        let view = SourceView::new("missing".into(), TreeKind::Class);

        assert_eq!(
            view.markdown(data(&model, &bundle)),
            "*No source for `missing`*"
        );
    }
}
```

Replace the throwaway-factory chrome/accent tests in `doc_view.rs` with:

```rust
#[test]
fn concrete_views_declare_the_existing_body_chrome() {
    let diagram =
        crate::class_diagram_view::ClassDiagramView::new("d".into(), "D".into());
    let classifier =
        crate::classifier_preview_view::ClassifierPreviewView::new(
            "order".into(),
            TreeKind::Class,
        );
    let source = crate::source_view::SourceView::new(
        "order".into(),
        TreeKind::Class,
    );

    assert_eq!(
        diagram.chrome(),
        BodyChrome {
            tool_dock: true,
            view_bar: true,
            right_dock: Some(Icon::SlidersHorizontal),
        }
    );
    for chrome in [classifier.chrome(), source.chrome()] {
        assert_eq!(
            chrome,
            BodyChrome {
                tool_dock: false,
                view_bar: false,
                right_dock: Some(Icon::SlidersHorizontal),
            }
        );
    }
}

#[test]
fn accents_come_from_self_identifying_views() {
    let classifier =
        crate::classifier_preview_view::ClassifierPreviewView::new(
            "status".into(),
            TreeKind::Enum,
        );
    let source = crate::source_view::SourceView::new(
        "status".into(),
        TreeKind::Enum,
    );

    assert_eq!(
        classifier.tab_accent(),
        crate::accent::tree_kind_color(TreeKind::Enum)
    );
    assert_eq!(
        source.tab_accent(),
        Some(crate::accent::bucket_color(
            crate::node_style::AccentBucket::None,
        ))
    );
}
```

Do not call a factory in chrome/accent tests.

- [ ] **Step 8: Adapt the temporary App-owned registry without downcasts**

Update the factory for this intermediate compiling stage:

```rust
pub fn make_view(tab: &DocTab) -> Box<dyn DocView> {
    match tab.kind {
        TabKind::Diagram => Box::new(
            crate::class_diagram_view::ClassDiagramView::new(
                tab.key.clone(),
                tab.title.clone(),
            ),
        ),
        TabKind::Classifier => Box::new(
            crate::classifier_preview_view::ClassifierPreviewView::new(
                tab.key.clone(),
                tab.node_kind,
            ),
        ),
        TabKind::Source => Box::new(
            crate::source_view::SourceView::new(
                tab.key.clone(),
                tab.node_kind,
            ),
        ),
    }
}
```

In `App`, add:

```rust
fn view_data(&self) -> crate::doc_view::ViewData<'_> {
    crate::doc_view::ViewData {
        model: self.session.model(),
        bundle: self.session.bundle(),
        revision: self.session.revision(),
    }
}
```

At each view call, construct `ViewData` from disjoint session borrows before
mutably borrowing `views`. Remove every `set_active`, `downcast_diagram`, and
source branch. `sync_active_tab` must:

1. reconcile the registry;
2. ensure the active live view exists;
3. query `view.chrome()` and call `body.apply_chrome`;
4. call `view.sync(cx, &body, data)`;
5. leave tree/status/conflict shell projections unchanged.

`refresh_doc_tabs` must query `tab_accent()` from the registered active live
view, falling back to `None`; it must not construct a view.

After either successful `EditorSession::apply_ops`, call:

```rust
if let Some(view) = self.views.get_mut(&active.id) {
    view.after_session_change(cx, &body, data, change);
}
```

- [ ] **Step 9: Verify there is no source special case or downcast seam**

Run:

```powershell
rtk cargo fmt --check
rtk cargo test -p waml-editor source_view::tests
rtk cargo test -p waml-editor doc_view::tests
rtk cargo test -p waml-editor class_diagram_view::tests
rtk cargo test -p waml-editor
rtk rg -n "as_any_mut|downcast_diagram|set_active\(|resolve_active|TabKind::Source" crates/waml-editor/src/app.rs crates/waml-editor/src/doc_view.rs
```

Expected: all tests PASS. The final search may find the one composition factory
match in `doc_view.rs` at this intermediate stage; it must find no App source
branch, downcast API, identity setter, or concrete mutation refresh.

- [ ] **Step 10: Commit live-view authority**

```powershell
rtk git add crates/waml-editor/src/doc_view.rs crates/waml-editor/src/class_diagram_view.rs crates/waml-editor/src/classifier_preview_view.rs crates/waml-editor/src/source_view.rs crates/waml-editor/src/app.rs
rtk git commit -m "refactor(editor): give live views document authority"
```

### Task 4: Introduce `DocumentHost` as the Only Tab and Live-View Owner

**Files:**
- Create: `crates/waml-editor/src/document_host.rs`
- Modify: `crates/waml-editor/src/main.rs:1-45`
- Modify: `crates/waml-editor/src/doc_view.rs:288-297`
- Modify: `crates/waml-editor/src/app.rs:540-605,636-803,1392-1477,1975-2888,3143-3206`
- Test: inline tests in `crates/waml-editor/src/document_host.rs`

**Interfaces:**
- Consumes: `OpenTabs`, `DocTab`, `DocTabs`, `DocView`, `ViewData`, `ViewOutcome`, `EditorSession`, and `SessionChange`.
- Produces: `DocumentHost`, `DocumentCommand`, the public API listed in **File Structure**, and the only `TabKind` factory.

- [ ] **Step 1: Write failing registry and live-projection tests**

Declare `mod document_host;` in `main.rs`. Create `document_host.rs` with the
imports, `DocumentCommand`, empty `DocumentHost`, and these tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use std::rc::Rc;

    struct ProbeView {
        chrome_calls: Rc<Cell<usize>>,
        accent_calls: Rc<Cell<usize>>,
    }

    impl DocView for ProbeView {
        fn sync(&mut self, _: &mut Cx, _: &BodyWidgets, _: ViewData<'_>) {
            unreachable!()
        }

        fn handle(
            &mut self,
            _: &mut Cx,
            _: &BodyWidgets,
            _: &Actions,
            _: ViewData<'_>,
        ) -> ViewOutcome {
            unreachable!()
        }

        fn chrome(&self) -> BodyChrome {
            self.chrome_calls.set(self.chrome_calls.get() + 1);
            BodyChrome {
                tool_dock: true,
                view_bar: false,
                right_dock: None,
            }
        }

        fn tab_accent(&self) -> Option<Vec4> {
            self.accent_calls.set(self.accent_calls.get() + 1);
            Some(vec4(0.1, 0.2, 0.3, 1.0))
        }
    }

    #[test]
    fn preview_replacement_drops_the_replaced_live_view() {
        let mut host = DocumentHost::default();
        host.tabs = OpenTabs::diagram_preview("orders", "Orders");
        let replaced = host.tabs.active;
        host.views.insert(
            replaced,
            Box::new(ProbeView {
                chrome_calls: Rc::new(Cell::new(0)),
                accent_calls: Rc::new(Cell::new(0)),
            }),
        );

        assert!(host.apply_command(DocumentCommand::Open {
            key: "customer".into(),
            title: "Customer".into(),
            node_kind: TreeKind::Class,
            persistent: false,
        }).0);

        assert!(!host.views.contains_key(&replaced));
        assert!(host.views.contains_key(&host.tabs.active));
    }

    #[test]
    fn close_reconciles_and_keeps_the_existing_right_then_left_fallback() {
        let mut host = DocumentHost::default();
        host.tabs = OpenTabs::diagram_preview("orders", "Orders");
        let orders = host.tabs.active;
        host.tabs.promote(orders);
        let customer =
            host.tabs
                .open_preview("customer", "Customer", TreeKind::Class);
        host.tabs.promote(customer);
        let source = host.tabs.open_source("order", "Order");
        host.reconcile_registry();

        assert!(host.apply_command(DocumentCommand::Close(customer)).0);
        assert_eq!(host.active_id(), source);
        assert!(!host.views.contains_key(&customer));

        assert!(host.apply_command(DocumentCommand::Close(source)).0);
        assert_eq!(host.active_id(), orders);
    }

    #[test]
    fn chrome_is_queried_from_the_registered_live_view() {
        let calls = Rc::new(Cell::new(0));
        let mut host = DocumentHost::default();
        host.tabs = OpenTabs::diagram_preview("orders", "Orders");
        host.views.insert(
            host.tabs.active,
            Box::new(ProbeView {
                chrome_calls: calls.clone(),
                accent_calls: calls.clone(),
            }),
        );

        assert_eq!(
            host.active_chrome(),
            BodyChrome {
                tool_dock: true,
                view_bar: false,
                right_dock: None,
            }
        );
        assert_eq!(calls.get(), 1);
        assert_eq!(
            host.active_accent(),
            Some(vec4(0.1, 0.2, 0.3, 1.0))
        );
        assert_eq!(calls.get(), 2);
        assert_eq!(host.views.len(), 1);
    }

    #[test]
    fn replacing_a_session_drops_views_even_when_tab_ids_repeat() {
        let mut host = DocumentHost::default();
        host.tabs = OpenTabs::diagram_preview("orders", "Old Orders");
        let repeated = host.tabs.active;
        host.views.insert(
            repeated,
            Box::new(ProbeView {
                chrome_calls: Rc::new(Cell::new(0)),
                accent_calls: Rc::new(Cell::new(0)),
            }),
        );

        let removed = host.replace_tabs_for_session(
            OpenTabs::diagram_preview("orders", "New Orders"),
        );

        assert_eq!(removed.len(), 1);
        assert_eq!(removed[0].0, repeated);
        assert_eq!(
            host.active_tab().map(|tab| tab.title.as_str()),
            Some("New Orders")
        );
        assert!(host.views.contains_key(&repeated));
    }
}
```

- [ ] **Step 2: Run the host tests to verify they fail**

```powershell
rtk cargo test -p waml-editor document_host::tests
```

Expected: FAIL because `DocumentHost` has no state or methods.

- [ ] **Step 3: Implement the host's pure state and registry core**

Add:

```rust
use crate::doc_tabs::{DocTab, DocTabs, OpenTabs, TabKind};
use crate::doc_view::{
    BodyChrome, BodyWidgets, DocView, ViewData, ViewOutcome,
};
use crate::editor_session::{EditorSession, SessionChange};
use crate::popup::base::{PopupResult};
use crate::tree::TreeKind;
use makepad_widgets::*;
use std::collections::{HashMap, HashSet};

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

#[derive(Default)]
pub struct DocumentHost {
    tabs: OpenTabs,
    views: HashMap<LiveId, Box<dyn DocView>>,
}

fn make_view(tab: &DocTab) -> Box<dyn DocView> {
    match tab.kind {
        TabKind::Diagram => Box::new(
            crate::class_diagram_view::ClassDiagramView::new(
                tab.key.clone(),
                tab.title.clone(),
            ),
        ),
        TabKind::Classifier => Box::new(
            crate::classifier_preview_view::ClassifierPreviewView::new(
                tab.key.clone(),
                tab.node_kind,
            ),
        ),
        TabKind::Source => Box::new(
            crate::source_view::SourceView::new(
                tab.key.clone(),
                tab.node_kind,
            ),
        ),
    }
}

impl DocumentHost {
    fn replace_tabs_for_session(
        &mut self,
        tabs: OpenTabs,
    ) -> Vec<(LiveId, Box<dyn DocView>)> {
        let removed = self.views.drain().collect();
        self.tabs = tabs;
        self.reconcile_registry();
        removed
    }

    fn reconcile_registry(&mut self) -> Vec<(LiveId, Box<dyn DocView>)> {
        let open: HashSet<LiveId> =
            self.tabs.tabs.iter().map(|tab| tab.id).collect();
        let stale: Vec<LiveId> = self
            .views
            .keys()
            .copied()
            .filter(|id| !open.contains(id))
            .collect();
        let removed = stale
            .into_iter()
            .filter_map(|id| self.views.remove(&id).map(|view| (id, view)))
            .collect();
        for tab in &self.tabs.tabs {
            self.views
                .entry(tab.id)
                .or_insert_with(|| make_view(tab));
        }
        removed
    }

    fn apply_command(
        &mut self,
        command: DocumentCommand,
    ) -> (bool, Vec<(LiveId, Box<dyn DocView>)>) {
        let before = self.tabs.clone();
        match command {
            DocumentCommand::Open {
                key,
                title,
                node_kind,
                persistent,
            } => {
                let id = self.tabs.open_preview(key, title, node_kind);
                if persistent {
                    self.tabs.promote(id);
                }
            }
            DocumentCommand::OpenSource { key, title } => {
                self.tabs.open_source(key, title);
            }
            DocumentCommand::Activate(id) => self.tabs.activate(id),
            DocumentCommand::Promote(id) => {
                self.tabs.activate(id);
                self.tabs.promote(id);
            }
            DocumentCommand::PromoteSubject(key) => {
                if let Some(id) = self
                    .tabs
                    .tabs
                    .iter()
                    .find(|tab| tab.key == key)
                    .map(|tab| tab.id)
                {
                    self.tabs.promote(id);
                }
            }
            DocumentCommand::Close(id) => self.tabs.close(id),
        }
        let removed = self.reconcile_registry();
        (self.tabs != before, removed)
    }

    pub fn active_tab(&self) -> Option<&DocTab> {
        self.tabs.active_tab()
    }

    pub fn tabs(&self) -> &[DocTab] {
        &self.tabs.tabs
    }

    pub fn active_id(&self) -> LiveId {
        self.tabs.active
    }

    pub fn active_chrome(&self) -> BodyChrome {
        self.views
            .get(&self.tabs.active)
            .map(|view| view.chrome())
            .unwrap_or(BodyChrome::HIDDEN)
    }

    pub fn active_accent(&self) -> Option<Vec4> {
        self.views
            .get(&self.tabs.active)
            .and_then(|view| view.tab_accent())
    }
}
```

Delete `make_view` from `doc_view.rs`. After this step, `document_host.rs` is
the only composition root matching `TabKind`.

- [ ] **Step 4: Implement the framework-facing transition tail**

Add a private `data` constructor, tab refresh, sync, dispatch, and transitions:

```rust
fn data(session: &EditorSession) -> ViewData<'_> {
    ViewData {
        model: session.model(),
        bundle: session.bundle(),
        revision: session.revision(),
    }
}

impl DocumentHost {
    fn refresh_tabs(&self, cx: &mut Cx, ui: &WidgetRef) {
        if let Some(mut tabs) = ui
            .widget(cx, ids!(doc_tabs))
            .borrow_mut::<DocTabs>()
        {
            tabs.set_tabs(cx, &self.tabs);
            tabs.set_active_accent(cx, self.active_accent());
        }
    }

    pub fn sync_active(
        &mut self,
        cx: &mut Cx,
        ui: &WidgetRef,
        session: &EditorSession,
    ) {
        self.reconcile_registry();
        let body = BodyWidgets::new(cx, ui);
        body.apply_chrome(cx, self.active_chrome());
        let active = self.tabs.active;
        if let Some(view) = self.views.get_mut(&active) {
            view.sync(cx, &body, data(session));
        }
    }

    fn finish_transition(
        &mut self,
        cx: &mut Cx,
        ui: &WidgetRef,
        session: &EditorSession,
        old_active: LiveId,
        mut removed: Vec<(LiveId, Box<dyn DocView>)>,
    ) {
        let body = BodyWidgets::new(cx, ui);
        let new_active = self.tabs.active;
        if old_active != new_active {
            if let Some((_, view)) =
                removed.iter_mut().find(|(id, _)| *id == old_active)
            {
                view.on_deactivate(cx, &body);
            } else if let Some(view) = self.views.get_mut(&old_active) {
                view.on_deactivate(cx, &body);
            }
            if let Some(view) = self.views.get_mut(&new_active) {
                view.on_activate(cx, &body);
            }
        }
        self.refresh_tabs(cx, ui);
        self.sync_active(cx, ui, session);
    }

    pub fn transition(
        &mut self,
        cx: &mut Cx,
        ui: &WidgetRef,
        session: &EditorSession,
        command: DocumentCommand,
    ) -> bool {
        let old_active = self.tabs.active;
        let (changed, removed) = self.apply_command(command);
        self.finish_transition(cx, ui, session, old_active, removed);
        changed
    }

    pub fn replace_for_session(
        &mut self,
        cx: &mut Cx,
        ui: &WidgetRef,
        session: &EditorSession,
        tabs: OpenTabs,
    ) -> bool {
        let old_active = self.tabs.active;
        let changed = self.tabs != tabs;
        // A newly opened bundle is a new identity domain. Even when it happens
        // to mint the same LiveId, no per-tab state may cross the boundary.
        let removed = self.replace_tabs_for_session(tabs);
        self.finish_transition(cx, ui, session, old_active, removed);
        changed
    }

    pub fn after_session_change(
        &mut self,
        cx: &mut Cx,
        ui: &WidgetRef,
        session: &EditorSession,
        change: SessionChange,
    ) {
        let body = BodyWidgets::new(cx, ui);
        if let Some(view) = self.views.get_mut(&self.tabs.active) {
            view.after_session_change(cx, &body, data(session), change);
        }
        self.refresh_tabs(cx, ui);
    }

    pub fn handle_active(
        &mut self,
        cx: &mut Cx,
        ui: &WidgetRef,
        actions: &Actions,
        session: &EditorSession,
    ) -> Option<ViewOutcome> {
        let body = BodyWidgets::new(cx, ui);
        self.views
            .get_mut(&self.tabs.active)
            .map(|view| view.handle(cx, &body, actions, data(session)))
    }

    pub fn on_active_popup_result(
        &mut self,
        cx: &mut Cx,
        ui: &WidgetRef,
        session: &EditorSession,
        tag: LiveId,
        result: PopupResult,
    ) -> Option<ViewOutcome> {
        let body = BodyWidgets::new(cx, ui);
        self.views.get_mut(&self.tabs.active).map(|view| {
            view.on_popup_result(cx, &body, data(session), tag, result)
        })
    }

    pub fn on_active_popup_armed(
        &mut self,
        cx: &mut Cx,
        ui: &WidgetRef,
        session: &EditorSession,
        tag: LiveId,
        id: Option<LiveId>,
    ) -> Option<ViewOutcome> {
        let body = BodyWidgets::new(cx, ui);
        self.views.get_mut(&self.tabs.active).map(|view| {
            view.on_popup_armed(cx, &body, data(session), tag, id)
        })
    }
}
```

- [ ] **Step 5: Replace App tab/view fields and every direct mutation**

Replace `tabs` and `views` fields with:

```rust
#[rust]
documents: crate::document_host::DocumentHost,
```

Use:

```rust
use crate::document_host::{DocumentCommand, DocumentHost};
```

Translate all mutations exactly:

| Current operation | Replacement |
|---|---|
| `tabs.open_preview` + optional `promote` | `documents.transition(..., DocumentCommand::Open { ... })` |
| `tabs.open_source` | `documents.transition(..., DocumentCommand::OpenSource { ... })` |
| `tabs.activate(id)` | `documents.transition(..., DocumentCommand::Activate(id))` |
| activate + promote | `documents.transition(..., DocumentCommand::Promote(id))` |
| promote matching key | `documents.transition(..., DocumentCommand::PromoteSubject(key))` |
| `tabs.close(id)` | `documents.transition(..., DocumentCommand::Close(id))` |
| complete model/tab seed | `documents.replace_for_session(..., OpenTabs::diagram_preview(...))` |
| diagram-less seed | `documents.replace_for_session(..., OpenTabs::default())` |

Resolve titles before sending an open command with this shell adapter:

```rust
fn transition_document(
    &mut self,
    cx: &mut Cx,
    key: &str,
    node_kind: crate::tree::TreeKind,
    persistent: bool,
) -> bool {
    let title = if node_kind == crate::tree::TreeKind::Diagram {
        self.session
            .model()
            .diagrams
            .iter()
            .find(|diagram| diagram.key == key)
            .map(|diagram| diagram.title.clone())
    } else {
        self.session
            .model()
            .nodes
            .iter()
            .find(|node| node.key == key)
            .map(|node| {
                node.concept
                    .title
                    .clone()
                    .unwrap_or_else(|| node.key.clone())
            })
    };
    let Some(title) = title else {
        return false;
    };
    let changed = self.documents.transition(
        cx,
        &self.ui,
        &self.session,
        DocumentCommand::Open {
            key: key.to_owned(),
            title,
            node_kind,
            persistent,
        },
    );
    self.sync_document_shell(cx);
    changed
}

fn sync_document_shell(&mut self, cx: &mut Cx) {
    let selected = self
        .documents
        .active_tab()
        .map(|tab| tab.key.clone());
    if let Some(mut tree) = self
        .ui
        .widget(cx, ids!(project_tree))
        .borrow_mut::<crate::tree_panel::ProjectTree>()
    {
        tree.set_selected_key(cx, selected);
    }
    self.sync_diagram_switcher_current(cx);
    self.sync_statusbar(cx);
    self.sync_conflict_badge(cx);
}
```

After a transition, `App` may refresh only these shell-global projections.
Delete App's `reconcile_views`, `sync_active_tab`, `refresh_doc_tabs`, and
direct registry access. Change `doc_switcher_items` to accept `&[DocTab]` and
use `documents.tabs()`.

- [ ] **Step 6: Verify host ownership**

```powershell
rtk cargo fmt --check
rtk cargo test -p waml-editor document_host::tests
rtk cargo test -p waml-editor doc_tabs::tests
rtk cargo test -p waml-editor
rtk rg -n "self\.tabs|self\.views|OpenTabs" crates/waml-editor/src/app.rs
rtk rg -n "match tab\.kind|TabKind::" crates/waml-editor/src
```

Expected: tests PASS; App has no mutable tab/view state; the only concrete
`TabKind` construction match is `document_host.rs` (matches inside pure tab
state and widget rendering remain valid).

- [ ] **Step 7: Commit host ownership**

```powershell
rtk git add crates/waml-editor/src/main.rs crates/waml-editor/src/document_host.rs crates/waml-editor/src/doc_view.rs crates/waml-editor/src/app.rs
rtk git commit -m "refactor(editor): centralize document hosting"
```

### Task 5: Unify Session Mutation and Apply View Outcomes

**Files:**
- Modify: `crates/waml-editor/src/app/actions.rs`
- Modify: `crates/waml-editor/src/app.rs:2226-2290,2713-2888`
- Modify: `crates/waml-editor/src/dock.rs:46-50`
- Modify: `crates/waml-editor/src/doc_view.rs:76-103,299-478`
- Modify: `crates/waml-editor/src/inspector_panel.rs:990-995`
- Test: inline tests in `crates/waml-editor/src/editor_session.rs`
- Test: existing conflict and view outcome tests

**Interfaces:**
- Consumes: `EditorSession::apply_ops`, `DocumentHost::after_session_change`, `DocumentHost::transition`, popup presentation helpers, and shell projections.
- Produces: `App::apply_session_ops(&mut self, &mut Cx, &[Op], &str) -> Option<SessionChange>` and `App::apply_view_outcome(&mut self, &mut Cx, ViewOutcome) -> ActionFlow`.

- [ ] **Step 1: Delete unused outcome channels and update the default test**

Delete `open_preview` and `open_right_dock` from `ViewOutcome`, delete their
relay branches and `right_dock_open_requested` tests, and retain exactly:

```rust
#[derive(Default)]
pub struct ViewOutcome {
    pub ops: Vec<Op>,
    pub popup: Option<PopupRequest>,
    pub promote_subject: Option<String>,
    pub close_active: bool,
    pub statusbar_dirty: bool,
}
```

Update the now-stale `DockEvent::Open` and `Inspector::open_dock` comments to
describe their remaining responsive-shell use:

```rust
/// The responsive shell forced a dock open, idempotently. Drives any state to
/// `Pinned` and never collapses.
Open,
```

```rust
/// Open the panel idempotently for responsive shell coordination. A no-op
/// when already open, so repeated layout reconciliation causes no redraw.
pub fn open_dock(&mut self, cx: &mut Cx) {
    self.apply_dock(cx, DockEvent::Open);
}
```

Update `view_outcome_default_is_all_empty` to assert these five fields only.

- [ ] **Step 2: Add the shared mutation helper**

In `app/actions.rs`, implement:

```rust
impl App {
    fn apply_session_ops(
        &mut self,
        cx: &mut Cx,
        ops: &[waml::ops::Op],
        error_label: &str,
    ) -> Option<crate::editor_session::SessionChange> {
        if ops.is_empty() {
            return None;
        }
        match self.session.apply_ops(ops) {
            Ok(change) => {
                self.documents.after_session_change(
                    cx,
                    &self.ui,
                    &self.session,
                    change,
                );
                if change.navigation_changed {
                    self.nav_kinds =
                        crate::nav::kinds_in_model(self.session.model());
                    self.refresh_nav(cx, false);
                }
                if change.conflicts_changed {
                    self.sync_conflict_badge(cx);
                }
                self.mark_dirty(cx);
                Some(change)
            }
            Err(error) => {
                log!("{error_label}: {error:?}");
                None
            }
        }
    }
}
```

A failed operation returns before invalidation and save scheduling.

- [ ] **Step 3: Rename and narrow outcome application**

Move `relay_outcome` to `app/actions.rs`, rename it, remove the `active`
parameter, and use:

```rust
fn apply_view_outcome(
    &mut self,
    cx: &mut Cx,
    outcome: crate::doc_view::ViewOutcome,
) -> ActionFlow {
    let mut flow = ActionFlow::Continue;
    self.apply_session_ops(cx, &outcome.ops, "place.set failed");

    if let Some(request) = outcome.popup {
        self.present_view_popup(cx, request);
        flow = ActionFlow::Consumed;
    }
    if let Some(key) = outcome.promote_subject {
        self.documents.transition(
            cx,
            &self.ui,
            &self.session,
            DocumentCommand::PromoteSubject(key),
        );
        self.sync_document_shell(cx);
        flow = ActionFlow::Consumed;
    }
    if outcome.close_active {
        let id = self.documents.active_id();
        self.documents.transition(
            cx,
            &self.ui,
            &self.session,
            DocumentCommand::Close(id),
        );
        self.sync_document_shell(cx);
        flow = ActionFlow::Consumed;
    }
    if outcome.statusbar_dirty {
        self.sync_statusbar(cx);
    }
    flow
}
```

Extract the existing popup `match` byte-for-byte into:

```rust
fn present_view_popup(
    &mut self,
    cx: &mut Cx,
    request: crate::doc_view::PopupRequest,
)
```

Keep the same anchors, bounds, tags, item composition, and radial/select/menu
open modes.

- [ ] **Step 4: Route conflict deletion through the same transaction**

Replace the conflict list's direct session call with:

```rust
if let Some(op) = place_rm_for(&diagram, &action) {
    if self
        .apply_session_ops(cx, &[op], "place.rm failed")
        .is_some()
    {
        let conflicts = self
            .ui
            .widget(cx, ids!(canvas))
            .borrow::<crate::canvas::ClassDiagramSurface>()
            .map(|canvas| canvas.conflicts())
            .unwrap_or_default();
        if conflicts.is_empty() {
            if let Some(mut popup) = self
                .ui
                .widget(cx, ids!(popup_root))
                .borrow_mut::<PopupRoot>()
            {
                popup.close(cx);
            }
        } else {
            self.open_conflict_list(cx, conflicts);
        }
    }
}
```

This preserves the conflict popup's keep-open/re-anchor behavior.

- [ ] **Step 5: Verify the shared transaction and outcome cleanup**

```powershell
rtk cargo fmt --check
rtk cargo test -p waml-editor editor_session::tests
rtk cargo test -p waml-editor app::tests::conflict_delete
rtk cargo test -p waml-editor doc_view::tests::view_outcome_default_is_all_empty
rtk cargo test -p waml-editor
rtk rg -n "session\.apply_ops|waml::ops::apply" crates/waml-editor/src
rtk rg -n "open_preview|open_right_dock|relay_outcome" crates/waml-editor/src
```

Expected: all tests PASS; App has one `session.apply_ops` site in
`apply_session_ops`; the removed channels and old relay name are absent.

- [ ] **Step 6: Commit the shared mutation flow**

```powershell
rtk git add crates/waml-editor/src/app.rs crates/waml-editor/src/app/actions.rs crates/waml-editor/src/doc_view.rs crates/waml-editor/src/dock.rs crates/waml-editor/src/inspector_panel.rs
rtk git commit -m "refactor(editor): unify mutation outcomes"
```

### Task 6: Extract the Explicit Two-Phase Action Coordinator

**Files:**
- Modify: `crates/waml-editor/src/app/actions.rs`
- Modify: `crates/waml-editor/src/app.rs:1908-2710`
- Test: inline tests in `crates/waml-editor/src/app/actions.rs`

**Interfaces:**
- Consumes: the three order constants from Task 1 and cohesive shell/document handlers.
- Produces: `App::handle_action_batch`, three ordered observer methods, fifteen exclusive handler methods returning `ActionFlow`, and a one-line `MatchEvent::handle_actions` forwarder.

- [ ] **Step 1: Wire the constants into the production coordinator**

Remove the temporary file-level allowance, replace `use super::App;` with
`use super::*;` so the moved handlers retain the parent module's popup/action
imports and private helper access, and implement:

```rust
impl App {
    pub(super) fn handle_action_batch(
        &mut self,
        cx: &mut Cx,
        actions: &Actions,
    ) {
        for observer in OBSERVER_ORDER {
            match observer {
                ObserverHandler::CaptionAndDocks => {
                    self.observe_caption_and_docks(cx, actions)
                }
                ObserverHandler::PopupResults => {
                    self.observe_popup_results(cx, actions)
                }
                ObserverHandler::ConflictList => {
                    self.observe_conflict_list(cx, actions)
                }
            }
        }

        for handler in EXCLUSIVE_ORDER {
            let flow = match handler {
                ExclusiveHandler::NavigationScope => {
                    self.handle_navigation_scope(cx, actions)
                }
                ExclusiveHandler::NavigationQuery => {
                    self.handle_navigation_query(cx, actions)
                }
                ExclusiveHandler::NavigationFilter => {
                    self.handle_navigation_filter(cx, actions)
                }
                ExclusiveHandler::TreeContextMenu => {
                    self.handle_tree_context_menu(cx, actions)
                }
                ExclusiveHandler::TreeDocumentOpen => {
                    self.handle_tree_document_open(cx, actions)
                }
                ExclusiveHandler::DiagramSwitcher => {
                    self.handle_diagram_switcher(cx, actions)
                }
                ExclusiveHandler::ConflictBadge => {
                    self.handle_conflict_badge(cx, actions)
                }
                ExclusiveHandler::ActiveDocumentView => {
                    self.handle_active_document_view(cx, actions)
                }
                ExclusiveHandler::LogoMenu => {
                    self.handle_logo_menu(cx, actions)
                }
                ExclusiveHandler::StartScreen => {
                    self.handle_start_screen_action(cx, actions)
                }
                ExclusiveHandler::ShortcutsOverlay => {
                    self.handle_shortcuts_overlay(cx, actions)
                }
                ExclusiveHandler::FontsOverlay => {
                    self.handle_fonts_overlay(cx, actions)
                }
                ExclusiveHandler::IconsOverlay => {
                    self.handle_icons_overlay(cx, actions)
                }
                ExclusiveHandler::ColorsOverlay => {
                    self.handle_colors_overlay(cx, actions)
                }
                ExclusiveHandler::DocumentTabs => {
                    self.handle_document_tabs(cx, actions)
                }
            };
            if flow == ActionFlow::Consumed {
                return;
            }
        }
    }
}
```

- [ ] **Step 2: Move the three non-exclusive blocks without changing behavior**

Move current `app.rs` lines 1977-2079 into
`observe_caption_and_docks`, lines 2081-2224 into
`observe_popup_results`, and lines 2226-2290 into
`observe_conflict_list`. Use these signatures:

```rust
fn observe_caption_and_docks(&mut self, cx: &mut Cx, actions: &Actions)
fn observe_popup_results(&mut self, cx: &mut Cx, actions: &Actions)
fn observe_conflict_list(&mut self, cx: &mut Cx, actions: &Actions)
```

Inside `observe_popup_results`, iterate `DOCUMENT_POPUP_RELAY_ORDER` for the
element picker and placement dial callbacks. The `PlaceDialArmed` arm must call
`documents.on_active_popup_armed` before the `PlaceDialClosed` arm calls
`documents.on_active_popup_result`. Feed every returned outcome to
`apply_view_outcome`. Global burger/logo/nav/node/doc-switcher results remain
shell code in this observer.

- [ ] **Step 3: Move each exclusive block to its matching handler**

Use the exact source mapping below. Each former `return;` becomes
`ActionFlow::Consumed`; the fallthrough tail returns `ActionFlow::Continue`.

| Handler | Existing `app.rs` block |
|---|---|
| `handle_navigation_scope` | 2292-2342 |
| `handle_navigation_query` | 2344-2354 |
| `handle_navigation_filter` | 2356-2412 |
| `handle_tree_context_menu` | 2414-2456 |
| `handle_tree_document_open` | 2458-2468 |
| `handle_diagram_switcher` | 2470-2495 |
| `handle_conflict_badge` | 2497-2515 |
| `handle_active_document_view` | 2517-2534 |
| `handle_logo_menu` | 2536-2574 |
| `handle_start_screen_action` | 2576-2612 |
| `handle_shortcuts_overlay` | 2614-2625 |
| `handle_fonts_overlay` | 2627-2636 |
| `handle_icons_overlay` | 2638-2647 |
| `handle_colors_overlay` | 2649-2658 |
| `handle_document_tabs` | 2660-2709 |

The active-view handler is:

```rust
fn handle_active_document_view(
    &mut self,
    cx: &mut Cx,
    actions: &Actions,
) -> ActionFlow {
    let Some(outcome) = self.documents.handle_active(
        cx,
        &self.ui,
        actions,
        &self.session,
    ) else {
        return ActionFlow::Continue;
    };
    self.apply_view_outcome(cx, outcome)
}
```

The document-tab handler must translate all four `DocTabsAction` variants to
`DocumentCommand` or the unchanged popup presentation; it must not mutate
`OpenTabs`.

- [ ] **Step 4: Reduce the trait method to one forwarder**

In `impl MatchEvent for App`, replace the old method with:

```rust
fn handle_actions(&mut self, cx: &mut Cx, actions: &Actions) {
    self.handle_action_batch(cx, actions);
}
```

- [ ] **Step 5: Verify action ordering and focused behavior**

```powershell
rtk cargo fmt --check
rtk cargo test -p waml-editor app::actions::tests
rtk cargo test -p waml-editor doc_tabs::tests
rtk cargo test -p waml-editor popup
rtk cargo test -p waml-editor
```

Expected: all tests PASS, including placement-dial armed-before-closed.

- [ ] **Step 6: Commit the action coordinator**

```powershell
rtk git add crates/waml-editor/src/app.rs crates/waml-editor/src/app/actions.rs
rtk git commit -m "refactor(editor): make action priority explicit"
```

### Task 7: Remove Bypasses and Obsolete Ownership Scaffolding

**Files:**
- Modify: `crates/waml-editor/src/app.rs`
- Modify: `crates/waml-editor/src/app/actions.rs`
- Modify: `crates/waml-editor/src/doc_view.rs`
- Modify: `crates/waml-editor/src/document_host.rs`
- Modify: `crates/waml-editor/tests/README.md`
- Test: all inline `waml-editor` tests

**Interfaces:**
- Consumes: the final session, host, view, outcome, and action interfaces.
- Produces: no compatibility aliases or duplicate entry points; compiler-visible ownership boundaries.

- [ ] **Step 1: Delete obsolete helpers and imports**

Delete App's old tab-mutating `open_document` (retain the new
`transition_document` command adapter), registry reconciliation helper, tab
refresh, active-tab sync, right-dock chrome helper, and any now-unused `HashMap`,
`HashSet`, `Model`, `OpenTabs`, `TabKind`, concrete view, or `BodyWidgets`
imports. Delete superseded tests of App-owned registry reconciliation; the
equivalent tests now live in `document_host.rs`.

- [ ] **Step 2: Make every document read go through the session**

Run:

```powershell
rtk rg -n "self\.model|self\.bundle|model: Model|bundle: Vec<" crates/waml-editor/src/app.rs
```

Expected: no matches. Any match must be rewritten to `self.session.model()` or
`self.session.bundle()`; do not add convenience fields.

- [ ] **Step 3: Prove tab and view authority has one entry point**

Run:

```powershell
rtk rg -n "self\.tabs|self\.views|tabs\.(open_preview|open_source|activate|promote|close)" crates/waml-editor/src/app.rs crates/waml-editor/src/app/actions.rs
rtk rg -n "as_any_mut|downcast|set_active\(|resolve_active|body_chrome\(|tab_accent\(active" crates/waml-editor/src
rtk rg -n "TabKind::Source" crates/waml-editor/src/app.rs crates/waml-editor/src/app
```

Expected: no matches. All tab mutations are `DocumentCommand`s, view refresh is
trait dispatch, and the shell has no source synchronization branch.

- [ ] **Step 4: Document the verification-of-record workflow**

Update `crates/waml-editor/tests/README.md` to add an “Editor ownership parity”
subsection containing the eight screenshot names from Tasks 1/8 and this
interaction checklist:

```text
open/replace/promote/activate/close tabs; close fallback; diagram switch;
source fallback; picker and placement-dial armed/closed order; conflict focus,
delete, keep-open and dismiss; burger/logo/node/nav/doc-switcher popups;
shortcuts/fonts/icons/colors overlays; wide/narrow left and right dock toggles;
browser debounce save and refresh restore; native save remains non-durable
```

- [ ] **Step 5: Run strict focused verification**

```powershell
rtk cargo fmt --check
rtk cargo test -p waml-editor
rtk cargo clippy -p waml-editor --all-targets --all-features -- -D warnings
```

Expected: all commands PASS with no warnings.

- [ ] **Step 6: Commit cleanup**

```powershell
rtk git add crates/waml-editor/src/app.rs crates/waml-editor/src/app/actions.rs crates/waml-editor/src/doc_view.rs crates/waml-editor/src/document_host.rs crates/waml-editor/tests/README.md
rtk git commit -m "refactor(editor): remove shell ownership bypasses"
```

### Task 8: Run Full Automated and Native UI-Parity Verification

**Files:**
- Verify only: entire workspace
- Compare local artifacts: `C:\tmp\editor-ownership-before\*.png`
- Create local artifacts: `C:\tmp\editor-ownership-after\*.png`

**Interfaces:**
- Consumes: the completed branch and the fixed-size native baseline from Task 1.
- Produces: passing workspace checks and a recorded human parity verdict; no repository file changes.

- [ ] **Step 1: Run every automated gate from a clean command boundary**

```powershell
rtk cargo fmt --check
rtk cargo test -p waml-editor
rtk cargo test --workspace
rtk cargo clippy --workspace --all-targets --all-features -- -D warnings
```

Expected: all four commands exit 0. Do not claim completion from an earlier
task's output.

- [ ] **Step 2: Capture the post-refactor screenshots**

Keep the Windows display scale unchanged from the baseline and run:

```powershell
rtk proxy pwsh -NoProfile -Command @'
$ErrorActionPreference = "Stop"
$editorOwnershipRoot = (Resolve-Path ".").Path
$editorOwnershipTarget = Join-Path $editorOwnershipRoot "target"
$editorOwnershipExe = Join-Path $editorOwnershipTarget "debug\waml-editor.exe"
$editorOwnershipCapture = Join-Path $editorOwnershipRoot "scripts\capture-window.ps1"
$editorOwnershipOut = "C:\tmp\editor-ownership-after"
New-Item -ItemType Directory -Force -Path $editorOwnershipOut | Out-Null

Add-Type @"
using System;
using System.Runtime.InteropServices;
public static class EditorOwnershipWindowAfter {
    [DllImport("user32.dll", SetLastError=true)]
    public static extern bool MoveWindow(
        IntPtr hWnd, int x, int y, int width, int height, bool repaint);
}
"@

& rtk cargo build -p waml-editor --target-dir $editorOwnershipTarget
if ($LASTEXITCODE -ne 0) { throw "waml-editor build failed" }

function Start-EditorOwnershipWindowAfter {
    param([string[]]$EditorOwnershipArgs)
    $editorOwnershipProcess = Start-Process `
        -FilePath $editorOwnershipExe `
        -ArgumentList $EditorOwnershipArgs `
        -WorkingDirectory $editorOwnershipRoot `
        -WindowStyle Normal `
        -PassThru
    $editorOwnershipDeadline = (Get-Date).AddSeconds(30)
    do {
        Start-Sleep -Milliseconds 200
        $editorOwnershipProcess.Refresh()
        if ($editorOwnershipProcess.HasExited) {
            throw "editor pid=$($editorOwnershipProcess.Id) exited before opening a window"
        }
    } while (
        $editorOwnershipProcess.MainWindowHandle -eq 0 -and
        (Get-Date) -lt $editorOwnershipDeadline
    )
    if ($editorOwnershipProcess.MainWindowHandle -eq 0) {
        throw "editor pid=$($editorOwnershipProcess.Id) opened no window"
    }
    [EditorOwnershipWindowAfter]::MoveWindow(
        $editorOwnershipProcess.MainWindowHandle, 40, 40, 1280, 900, $true) | Out-Null
    Start-Sleep -Milliseconds 500
    return $editorOwnershipProcess
}

function Save-EditorOwnershipShotAfter {
    param(
        [System.Diagnostics.Process]$EditorOwnershipProcess,
        [string]$EditorOwnershipName
    )
    & rtk pwsh -File $editorOwnershipCapture `
        -Out (Join-Path $editorOwnershipOut "$EditorOwnershipName.png") `
        -ProcessId $editorOwnershipProcess.Id
    if ($LASTEXITCODE -ne 0) { throw "capture failed: $EditorOwnershipName" }
}

$editorOwnershipStart = Start-EditorOwnershipWindowAfter -EditorOwnershipArgs @()
try {
    $null = Read-Host "Confirm the empty start screen is settled; press Enter"
    Save-EditorOwnershipShotAfter $editorOwnershipStart "start-screen"
}
finally {
    Stop-Process -Id $editorOwnershipStart.Id -ErrorAction SilentlyContinue
}

$editorOwnershipFixture =
    (Resolve-Path "crates/waml-editor/tests/fixtures/mini").Path
$editorOwnershipEditor =
    Start-EditorOwnershipWindowAfter -EditorOwnershipArgs @($editorOwnershipFixture)
try {
    $editorOwnershipPrompts = @(
        @("class-diagram", "Show Orders diagram with both docks open"),
        @("classifier-preview", "Open Customer as the active classifier preview"),
        @("source-view", "Open Customer context menu and choose View Source"),
        @("tab-switching", "Pin Orders and Customer, then activate Orders from the tab strip"),
        @("popup", "Open the burger menu without moving or resizing the window"),
        @("overlay", "Open the shortcuts overlay"),
        @("docks-closed", "Return to Orders and close both left and right docks")
    )
    foreach ($editorOwnershipEntry in $editorOwnershipPrompts) {
        $null = Read-Host "$($editorOwnershipEntry[1]); press Enter"
        Save-EditorOwnershipShotAfter $editorOwnershipEditor $editorOwnershipEntry[0]
    }
}
finally {
    Stop-Process -Id $editorOwnershipEditor.Id -ErrorAction SilentlyContinue
}
'@
```

Expected: these eight files:

```text
start-screen.png
class-diagram.png
classifier-preview.png
source-view.png
tab-switching.png
popup.png
overlay.png
docks-closed.png
```

- [ ] **Step 3: Verify identical native dimensions before visual review**

```powershell
rtk proxy pwsh -NoProfile -Command @'
Add-Type -AssemblyName System.Drawing
$editorOwnershipBefore = "C:\tmp\editor-ownership-before"
$editorOwnershipAfter = "C:\tmp\editor-ownership-after"
$editorOwnershipNames = @(
    "start-screen",
    "class-diagram",
    "classifier-preview",
    "source-view",
    "tab-switching",
    "popup",
    "overlay",
    "docks-closed"
)
foreach ($editorOwnershipName in $editorOwnershipNames) {
    $editorOwnershipBeforeImage = [Drawing.Image]::FromFile(
        (Join-Path $editorOwnershipBefore "$editorOwnershipName.png"))
    $editorOwnershipAfterImage = [Drawing.Image]::FromFile(
        (Join-Path $editorOwnershipAfter "$editorOwnershipName.png"))
    try {
        if (
            $editorOwnershipBeforeImage.Width -ne $editorOwnershipAfterImage.Width -or
            $editorOwnershipBeforeImage.Height -ne $editorOwnershipAfterImage.Height
        ) {
            throw "$editorOwnershipName dimensions differ: before=$($editorOwnershipBeforeImage.Width)x$($editorOwnershipBeforeImage.Height), after=$($editorOwnershipAfterImage.Width)x$($editorOwnershipAfterImage.Height)"
        }
        Write-Output "$editorOwnershipName $($editorOwnershipAfterImage.Width)x$($editorOwnershipAfterImage.Height)"
    }
    finally {
        $editorOwnershipBeforeImage.Dispose()
        $editorOwnershipAfterImage.Dispose()
    }
}
'@
```

Expected: eight matching dimension lines and no exception.

- [ ] **Step 4: Perform native-resolution side-by-side review**

For each before/after pair, inspect at 100% zoom and confirm:

- identical caption bands, tab height/order/italic preview styling, active accent, and close affordances;
- identical start screen, tree selection, diagram canvas/camera, classifier preview, raw source text, statusbar, tool dock, view bar, inspector, and conflict badge;
- identical burger popup placement, overlay bounds, scrims, and z-order;
- identical left/right dock visibility, widths, button glyphs/lit state, and tab-strip top-rule reach;
- no flash of the wrong body surface or stale view-owned chrome during tab switches.

Any unexplained difference blocks completion. Fix it in the owning task, rerun
that task's focused tests, then repeat all of Task 8.

- [ ] **Step 5: Exercise temporal interaction parity**

In one native `mini` session, perform this exact sequence:

1. Open Orders, pin it, preview Customer, pin it, open Customer source, switch
   among all three, close the active source, then close the rightmost and
   leftmost remaining tabs; verify the same right-then-left fallback.
2. Cycle the diagram switcher, open the narrow document switcher, and activate
   a tab from it.
3. Open/dismiss burger, logo, tree node, nav scope, nav filter, picker, and
   conflict popups; commit and dismiss where both paths exist.
4. Drag-to-place through dwell, armed preview, commit, and cancel. Verify the
   committed scene refresh holds the camera.
5. Delete a placement from the conflict list; verify the list stays open and
   re-anchors while conflicts remain, then dismisses when empty.
6. Open/dismiss shortcuts, fonts, icons, and colors overlays.
7. Toggle both docks in wide and narrow layouts and confirm mutual exclusion in
   narrow mode.
8. Wait three seconds after a mutation and confirm no new native persistence UI
   or durable-save claim appears.

Expected: no interaction, event-priority, popup, tab, dock, overlay, camera, or
save-timing difference from the baseline.

- [ ] **Step 6: Confirm repository state and hand off**

```powershell
rtk git status --short
rtk git log --oneline -8
```

Expected: no uncommitted implementation changes and one independently
reviewable commit per Task 1-7. The screenshot artifacts remain under `C:\tmp`
and are not committed.
