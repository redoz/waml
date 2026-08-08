# Typed UI Regression Testing Design

**Status:** Proposed for implementation planning

**Date:** 2026-08-08

## Purpose

WAML needs automated tests for the assembled editor. Existing tests cover the
model, parsing, layout, history, and many event-routing units. They do not run
enough complete user journeys through the real Makepad widget tree and event
loop. Manual UI driving and screenshots are therefore still the verification
of record for many navigation and application-behavior changes.

This design adds a typed Rust DSL for semantic UI regression tests. A test
describes a WAML user task, such as opening a diagram or changing the active
document view. It does not describe widget IDs, mouse coordinates, database
operations, or other implementation details.

The work will ship as small vertical slices. Each slice must add useful CI
protection before the next slice starts.

## Goals

- Run complete editor journeys through the real Makepad widget tree and event
  path.
- Give scenario authors a small, typed, WAML-specific Rust API.
- Keep selectors, waits, retries, coordinates, and raw Makepad events out of
  scenario files.
- Make each failure identify the failed semantic step and preserve useful
  evidence.
- Stage every fixture so a UI test cannot change committed fixture files.
- Add coverage in independently mergeable slices.
- Run the first required UI gate with the existing Linux CI worker.
- Keep one scenario implementation usable in headless CI and visible Studio
  diagnosis.

## Non-goals

- Do not add Gherkin, YAML, JSON, or another external scenario language.
- Do not build a generic automation framework inside WAML.
- Do not replace existing unit, integration, property, or golden tests.
- Do not use Windows UI Automation, Appium, WinAppDriver, or coordinate-only
  desktop automation as the primary driver.
- Do not make screenshots the primary correctness signal.
- Do not expose all internal editor state through the public DSL.
- Do not make Windows headless UI execution a condition for the first slice.
- Do not add browser UI coverage for behavior that the native semantic suite
  already covers.

## Fixed decisions

- The public scenario language is typed Rust now and for future work.
- `makepad-test` is the low-level driver.
- WAML owns a semantic DSL above `makepad-test`.
- Scenario tests cannot call `makepad-test` directly.
- The first protected area is main application navigation and behavior.
- Delivery uses vertical slices instead of one large test-automation change.
- The first CI target is Linux headless execution. Windows keeps its existing
  non-UI gates until its Makepad headless backend is complete.

## Architecture

The system has four layers:

```text
WAML scenario
    -> WAML semantic DSL
        -> WAML UI adapters
            -> makepad-test primitives
```

### Scenario layer

Scenario files contain only user tasks and observable WAML outcomes. They use
the `#[waml_ui_test]` attribute and the `WamlApp` facade. They do not import
`makepad_test`.

The initial authoring form is:

```rust,ignore
use waml_ui_test::{
    waml_ui_test, DiagramName, ViewKind, WamlApp, WorkspaceFixture,
};

#[waml_ui_test(workspace = Mini)]
fn open_and_switch_document_views(mut app: WamlApp) {
    app.ensure_workspace_open(WorkspaceFixture::MINI)
        .ensure_diagram_open(DiagramName::ORDERS)
        .expect_active_diagram(DiagramName::ORDERS)
        .switch_active_document_to(ViewKind::Source)
        .expect_active_view(ViewKind::Source)
        .switch_active_document_to(ViewKind::Diagram)
        .expect_active_view(ViewKind::Diagram);
}
```

`WorkspaceFixture`, `DiagramName`, and similar values are typed newtypes with
well-known constants for committed fixtures. A later test can create a new
typed value when it creates data during the scenario. The DSL must not use a
closed enum that makes every new fixture or document require a release of the
test library.

### Semantic DSL layer

`WamlApp` is the public facade. Its methods are grouped internally by domain:

- Workspace: load and recognize a workspace.
- Documents: open, activate, close, and navigate documents and tabs.
- Chrome: use menus, popups, overlays, and docks.
- Diagram: select, drag, cancel, commit, pan, zoom, and inspect diagrams.
- Source: edit, select, navigate, and inspect source documents.
- History: undo, redo, and move through view history.

The first slice implements only the workspace and document-view vocabulary
needed by its first journey. A domain module must not be added before a merged
scenario needs it.

The public DSL has three operation classes:

- `ensure_*` establishes an idempotent precondition. It returns immediately
  when the condition is already true. Otherwise, it performs the minimum
  semantic action and waits for the condition.
- An imperative verb, such as `open_diagram` or
  `switch_active_document_to`, performs a deliberate user action. It fails
  when that action is not available.
- `expect_*` observes without mutation and fails when the expected state is
  not present.

This distinction prevents an assertion from silently repairing the state that
it was meant to verify.

Every public operation returns `&mut WamlApp` on success so a scenario can use
a readable chain. A failure ends the test with a `WamlUiError` that includes
the semantic operation, expected state, observed state, and low-level evidence.

### UI adapter layer

Adapters translate semantic operations into `makepad-test` selectors,
interactions, and waits. An adapter may know Live IDs, widget types, text,
geometry, and Makepad event details. This information is private to the test
support crate.

Adapters follow these rules:

- Prefer a stable Live ID over visible text.
- Prefer a semantic state value over geometry.
- Do not use `nth` when a stable identity can be added to the application.
- Use a coordinate only for an interaction whose meaning is inherently
  geometric, such as a canvas drag.
- Wait for a semantic postcondition instead of sleeping for a fixed time.
- Keep one adapter as the owner of each selector or widget contract.
- Report missing or duplicate selector matches as testability defects.

Stable widget identity and observable widget state become application test
contracts. A production widget change that breaks a contract must either
preserve the contract or update its single owning adapter.

### Makepad driver layer

`makepad-test` owns application launch, Studio protocol communication, widget
snapshots, input injection, polling, screenshots, and base failure artifacts.
WAML does not copy those facilities.

The pinned Makepad revision already contains `makepad-test`. The initial
framework gap is launch arguments: `TestConfig` currently sends an empty
argument list to the Studio `Run` request. WAML needs a staged workspace path
and a required window title.

The Makepad change adds an application argument list to `TestConfig` and sends
it unchanged in headless and visible modes. Framework tests must prove both
paths before WAML updates its Makepad pin.

## Package boundaries

The intended WAML test-support packages are:

- `waml-ui-test`: normal Rust library. It owns `WamlApp`, typed domain values,
  semantic errors, traces, fixture staging, UI adapters, and the launch runner.
- `waml-ui-test-macros`: proc-macro library. It owns only expansion of
  `#[waml_ui_test(workspace = Mini)]` into a serial Rust test that calls the
  `waml-ui-test` runner.

`waml-ui-test` re-exports the attribute macro so scenario files need one test
support dependency. Both packages are private workspace support crates. They
are not part of the public WAML product API.

The macro accepts a workspace identifier from the compiled fixture catalog.
It does not accept a free-form fixture path. A test that creates a workspace
dynamically uses a separate typed runner API instead of weakening the common
attribute syntax.

The `waml-editor` UI scenarios remain in its `tests/` directory. This keeps the
test target attached to the application package that `makepad-test` launches.

## Test launch and isolation

For each test, the WAML runner performs this sequence:

1. Resolve the workspace identifier through the compiled fixture catalog.
2. Create `target/waml-ui-test/<test-name>/workspace` from the committed
   fixture.
3. Remove an older staged workspace for the same test before the copy.
4. Preserve the committed fixture as a read-only input.
5. Build launch arguments with the staged path and
   `--title ui-<test-name>`.
6. Start `waml-editor` through `makepad-test`.
7. Wait for the semantic application-ready condition.
8. Construct `WamlApp` and start the semantic trace.
9. Run the scenario.
10. Remove the staged workspace after success.
11. Preserve the staged workspace and all evidence after failure.

The first application-ready condition is a visible workspace shell plus a
project tree that identifies the expected fixture root. Window creation or a
nonzero window handle alone is not sufficient.

All UI scenarios run serially. The runner and CI command both enforce this
rule. A test must not find, reuse, or stop an application process by name.

## Semantic step execution

Every DSL operation uses the same internal algorithm:

1. Record the operation name and semantic input.
2. Observe the current semantic state.
3. For `ensure_*`, return success if the required state is present.
4. Resolve exactly one actionable UI target through its adapter.
5. Perform the minimum interaction.
6. Poll for the semantic postcondition until the operation timeout.
7. Record the observed result.
8. On failure, capture all evidence before the application stops.

The default timeout comes from `makepad-test`. A domain operation can use a
different timeout only when it documents a real asynchronous boundary, such
as workspace loading. Scenario code cannot set timeouts or add sleeps.

## Errors and evidence

`WamlUiError` reports failures in WAML terms. Its display form contains:

- Test name.
- Ordered semantic step number.
- Operation and typed inputs.
- Expected semantic postcondition.
- Last observed semantic state.
- Adapter target and selector evidence.
- Timeout or underlying driver error.
- Artifact directory.

Example:

```text
Step 4: switch active document to Source failed
Expected: active view is Source for Orders
Observed: active view remained Diagram; source control was disabled
Artifacts: target/waml-ui-test/open_and_switch_document_views/
```

The artifact directory contains:

- `semantic-trace.txt`: readable ordered steps and outcomes.
- `semantic-trace.json`: structured form for future CI tooling.
- `failure.txt`: final error.
- `logs.txt`: application and driver logs.
- `widget-snapshot.json`: structured Makepad widget state.
- `widget-tree.txt`: compact tree dump.
- `failure-screenshot.png`: diagnostic image when capture succeeds.
- `workspace/`: exact staged workspace at failure time.

A screenshot is evidence, not the assertion. A test passes from semantic state
and application outcomes.

## First vertical slice

The first slice has two merge boundaries.

### Makepad launch support

- Add `args: Vec<String>` to `makepad_test::TestConfig` with an empty default.
- Forward `args` to the Studio `Run` request in headless mode.
- Forward the same `args` in visible Studio mode.
- Add framework tests for default-empty, headless forwarding, and visible
  forwarding behavior.
- Land the Makepad change as a focused commit.

### WAML harness and navigation journey

- Update WAML to the Makepad revision that contains launch-argument support.
- Add the private test-support packages and fixture catalog.
- Add the workspace and document-view DSL needed by the first scenario.
- Add stable widget identities or observable state only where the adapters
  cannot express the journey reliably with existing contracts.
- Stage and launch the `mini` fixture with a unique title.
- Test workspace readiness, opening the Orders diagram, and switching the
  active document between Diagram and Source views.
- Add the serial UI target to Linux CI as a required check.
- Document visible Studio diagnosis for the same scenario.

This slice detects failures in application assembly, fixture loading, project
tree routing, document activation, view switching, and semantic readiness.

## CI strategy

The initial command is a dedicated serial target:

```bash
cargo test -p waml-editor --test ui -- --test-threads=1
```

It runs on the Linux worker after normal Rust tests. It is a required check.
The first implementation task must qualify `makepad-test` headless execution
on the repository's current Linux CI image. If qualification exposes a generic
Makepad defect, the fix belongs in the Makepad fork. WAML must not add a
product-only driver workaround.

Windows continues to run all current non-UI checks. A later slice completes
the Makepad Windows headless backend and adds the identical semantic suite to
the Windows matrix. Scenario code and domain behavior must remain shared.

The existing Playwright browser check continues to cover browser boot, API
security, and save round trips. It does not duplicate native semantic journeys
through canvas coordinates.

## Later vertical slices

### Document navigation

- Open more than one document from the project tree.
- Reuse an existing tab instead of creating a duplicate.
- Activate active and inactive tabs.
- Close active and inactive tabs.
- Verify close fallback.
- Switch diagram, source, and generic views.
- Navigate backward and forward through view history.

### Application chrome and ownership

- Open and close menus, popups, overlays, and docks.
- Verify Escape and outside-click behavior.
- Verify narrow and wide dock states.
- Verify overlays consume scroll and pointer input.
- Run global shortcuts through real keyboard events.

### Stateful editor behavior

- Undo and redo.
- Preserve selection across supported tab and scene changes.
- Change inspector properties.
- Focus and revalidate conflicts.
- Save changes and restore state after refresh.
- Cancel pending timers and interactions on scene changes.

### Diagram and source interaction

- Select, drag, retarget, cancel, and commit diagram interactions.
- Pan and zoom.
- Retain the camera across scene refresh.
- Edit and select Markdown source.
- Observe diagnostics and scroll restoration.

### Platform and visual coverage

- Complete and qualify Makepad headless execution on Windows.
- Run the same semantic suite on Windows CI.
- Add a small visual-golden suite only for typography, shaders, diagram
  rendering, and other output that semantic state cannot describe.

## Slice delivery contract

Each later slice must:

1. Add only the typed domain vocabulary that its journeys need.
2. Put all Makepad details in private adapters.
3. Add at least one complete user journey.
4. Verify meaningful postconditions, not only attempted clicks.
5. Add the journey to required CI before merge.
6. Remove or mark the corresponding manual check as exploratory.
7. Document public DSL additions and add contract tests for their semantics.

A slice is complete only when:

- Scenario files contain no raw selectors, widget IDs, coordinates, sleeps,
  or Makepad events.
- Repeated isolated and serial-suite runs pass.
- Failures identify the semantic step and preserve all available evidence.
- The same test source works in headless and visible modes.
- Existing unit and integration tests remain green.

## Risks and controls

### The DSL becomes a second UI framework

Control: keep the DSL WAML-specific and add vocabulary only for a merged user
journey. Delegate mechanics to `makepad-test`.

### Tests hide regressions by repairing state

Control: use `ensure_*` only for preconditions. Use imperative actions for the
behavior under test and `expect_*` for non-mutating verification.

### Selector churn makes scenarios fragile

Control: centralize every selector in one adapter, prefer stable Live IDs, and
treat duplicate matches as defects.

### Headless behavior differs from the native application

Control: send the same Makepad events through the same widget tree and keep a
visible Studio mode for diagnosis. Add Windows execution when the backend is
ready. Keep a small visual suite for renderer-only risks.

### Fixture tests change committed files

Control: stage a new per-test copy and launch only that copy. Preserve failed
copies for diagnosis.

### UI tests make the normal suite slow or flaky

Control: use a dedicated serial target, semantic waits, deterministic fixtures,
and narrow vertical journeys. Do not use fixed sleeps.

## Success criteria

The first slice succeeds when all of these conditions are true:

- A WAML scenario uses only the typed semantic DSL.
- The scenario launches a staged `mini` workspace with a unique title.
- It opens the Orders diagram and switches between Diagram and Source views.
- It verifies each result from semantic widget state.
- It runs through normal `cargo test` in Linux headless CI.
- The same scenario can run visibly through Studio without source changes.
- A forced failure produces the semantic trace, widget evidence, screenshot,
  logs, and staged workspace.
- The covered navigation steps are no longer part of verification-of-record
  manual testing.
