# Typed UI Regression Testing Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add the first continuously integrated WAML editor journey through a typed Rust semantic DSL, covering fixture readiness, diagram activation, and Diagram/Source view switching.

**Architecture:** Extend `makepad-test` with launch arguments and a generic custom-widget semantic-item hook. Build a private WAML test-support library and proc macro above those primitives. Scenario files call only `WamlApp` domain operations; private adapters own Makepad selectors and interactions.

**Tech Stack:** Rust 1.80+, Makepad Studio protocol, `makepad-test`, proc macros with `syn`/`quote`, `serde`/`serde_json`, Cargo integration tests, GitHub Actions Linux runner.

## Global Constraints

- Never edit `C:\dev\waml` or `C:\dev\makepad` directly. Use an isolated git worktree for each repository.
- Create the Makepad worktree from `origin/waml`, because WAML pins that integration line. Do not include the unrelated untracked `C:\dev\makepad\docs\superpowers\plans\origin` path.
- Keep WAML work on a worktree based on `origin/main` with the approved specification and this plan applied.
- Use ASD-STE100 Simplified Technical English in code comments, errors, and documentation.
- Scenario code is typed Rust now and in future work.
- Scenario files must not import `makepad_test` or contain selectors, widget IDs, coordinates, sleeps, raw Makepad events, or timeout values.
- `ensure_*` establishes an idempotent precondition, imperative verbs perform an action, and `expect_*` observes without mutation.
- Serial execution is an initial CI policy only. Paths, titles, process ownership, and connection resources must remain safe for later process-level sharding.
- Every editor launch must pass a short unique `--title` slug.
- Committed fixtures are read-only inputs. Every run launches a staged copy under `target/waml-ui-test`.
- The first runtime UI gate is Linux-only. Windows must compile all support code but must not run the headless UI target.
- Use `rtk` for every shell command.
- Run non-UI Makepad checks from the shell. Use Makepad Studio remote control for visible application diagnosis.
- Do not push the Makepad branch until the user approves the external write. The WAML git pin can change only after the Makepad commit is reachable from `https://github.com/redoz/makepad.git`.

---

## File Structure

### Makepad repository

- `libs/makepad_test/src/runtime.rs`: own `TestConfig::args` and build one shared Studio `Run` request for headless and visible modes.
- `libs/makepad_test/README.md`: document configured application arguments.
- `libs/makepad_test/GUIDE.md`: document application arguments and custom-drawn semantic controls.
- `widgets/src/widget.rs`: define `WidgetSemanticItem` and the default-empty `Widget::semantic_items` hook.
- `widgets/src/widget_tree.rs`: merge custom semantic items into normal `WidgetSnapshot` responses and test conversion.

### WAML repository

- `Cargo.toml`: register `waml-ui-test` and `waml-ui-test-macros`; pin Makepad packages to the reviewed Makepad commit.
- `Cargo.lock`: record the new support crates and Makepad revision.
- `crates/waml-ui-test-macros/Cargo.toml`: proc-macro crate manifest.
- `crates/waml-ui-test-macros/src/lib.rs`: parse `#[waml_ui_test(workspace = Mini)]` and expand the Rust test wrapper.
- `crates/waml-ui-test/Cargo.toml`: semantic test-support library manifest.
- `crates/waml-ui-test/src/lib.rs`: public typed DSL exports and private macro runner exports.
- `crates/waml-ui-test/src/config.rs`: `ScenarioConfig`, `WorkspaceFixture`, fixture descriptors, run identity, and short title.
- `crates/waml-ui-test/src/fixture.rs`: safe fixture staging, recursive copy, and owned-path cleanup.
- `crates/waml-ui-test/src/trace.rs`: persisted semantic step records in text and JSON.
- `crates/waml-ui-test/src/error.rs`: `OperationFailure` and `WamlUiError` formatting.
- `crates/waml-ui-test/src/run.rs`: one launch/staging/tracing/cleanup lifecycle around `makepad_test::run_with_config`.
- `crates/waml-ui-test/src/app.rs`: `WamlApp`, the semantic execution envelope, and public domain methods.
- `crates/waml-ui-test/src/domain.rs`: `DiagramName` and `ViewKind`.
- `crates/waml-ui-test/src/adapters/mod.rs`: private adapter module boundary.
- `crates/waml-ui-test/src/adapters/workspace.rs`: workspace-ready observation.
- `crates/waml-ui-test/src/adapters/documents.rs`: diagram-row, active-diagram, view-toggle, and active-view operations.
- `crates/waml-editor/Cargo.toml`: add the feature-gated UI integration target and support dependency.
- `crates/waml-editor/src/tree_layout.rs`: expose the current viewport rectangle to semantic-item generation.
- `crates/waml-editor/src/tree_panel.rs`: emit semantic items for visible project-tree rows.
- `crates/waml-editor/tests/ui.rs`: contain only the first typed semantic scenario.
- `crates/waml-editor/tests/README.md`: replace covered manual navigation checks with the automated command and visible diagnosis instructions.
- `.github/workflows/ci.yml`: run the UI target serially on Linux as a required step.

---

### Task 1: Forward Makepad application arguments

**Repository:** Makepad worktree from `origin/waml`

**Files:**
- Modify: `libs/makepad_test/src/runtime.rs:73-139`
- Modify: `libs/makepad_test/src/runtime.rs:1147-1208`
- Modify: `libs/makepad_test/src/runtime.rs:1519-1612`
- Modify: `libs/makepad_test/README.md`
- Modify: `libs/makepad_test/GUIDE.md`

**Interfaces:**
- Produces: `pub args: Vec<String>` on `makepad_test::TestConfig`.
- Produces: private `fn run_request(config: &TestConfig, mount: String) -> ClientToHub` used by both launch modes.
- Preserves: `TestConfig::new` and `TestConfig::current_package` signatures.
- Preserves: an empty argument list by default.

- [ ] **Step 1: Add failing configuration and request tests**

Add these tests to `runtime.rs` and import `run_request` plus `ClientToHub` in the test module:

```rust
#[test]
fn config_defaults_to_no_application_arguments() {
    let config = TestConfig::new("/tmp/example", "makepad-example", "ui::test").unwrap();
    assert!(config.args.is_empty());
}

#[test]
fn run_request_forwards_mount_package_and_arguments() {
    let mut config =
        TestConfig::new("/tmp/example", "makepad-example", "ui::test").unwrap();
    config.args = vec![
        "tests/fixtures/mini".to_string(),
        "--title".to_string(),
        "ui-1-nav".to_string(),
    ];

    let ClientToHub::Run {
        mount,
        process,
        args,
        ..
    } = run_request(&config, "visible-mount".to_string())
    else {
        panic!("expected Run request");
    };

    assert_eq!(mount, "visible-mount");
    assert_eq!(process, "makepad-example");
    assert_eq!(args, config.args);
}
```

- [ ] **Step 2: Run the tests and confirm the red state**

Run:

```powershell
rtk cargo test -p makepad-test config_defaults_to_no_application_arguments
rtk cargo test -p makepad-test run_request_forwards_mount_package_and_arguments
```

Expected: compilation fails because `TestConfig::args` and `run_request` do not exist.

- [ ] **Step 3: Add the minimal shared request builder**

Add `args` after `artifacts_dir`, initialize it with `Vec::new()`, and add:

```rust
fn run_request(config: &TestConfig, mount: String) -> ClientToHub {
    ClientToHub::Run {
        mount,
        process: config.package_name.clone(),
        args: config.args.clone(),
        standalone: None,
        env: Some(config.env.clone()),
        buildbox: None,
    }
}
```

Replace both literal `ClientToHub::Run` values in `start_headless_app` and
`start_visible_app` with `run_request(config, mount)`.

- [ ] **Step 4: Run the focused and package tests**

Run:

```powershell
rtk cargo test -p makepad-test config_defaults_to_no_application_arguments
rtk cargo test -p makepad-test run_request_forwards_mount_package_and_arguments
rtk cargo test -p makepad-test
```

Expected: all commands pass.

- [ ] **Step 5: Document the argument contract**

Add to `README.md` and `GUIDE.md`:

```rust,ignore
let mut config = TestConfig::current_package(
    env!("CARGO_MANIFEST_DIR"),
    env!("CARGO_PKG_NAME"),
    "ui::fixture",
)?;
config.args = vec!["tests/fixtures/mini".into(), "--title".into(), "ui-mini".into()];
run_with_config(config, |app| {
    app.locator(Selector::id("main_window")).wait_visible();
})
```

State that headless and visible modes receive the same arguments unchanged.

- [ ] **Step 6: Format, verify, and commit**

Run:

```powershell
rtk cargo fmt --all -- --check
rtk cargo test -p makepad-test
rtk git diff --check
rtk git add libs/makepad_test/src/runtime.rs libs/makepad_test/README.md libs/makepad_test/GUIDE.md
rtk git commit -m "feat(test): forward application arguments"
```

Expected: formatting and tests pass; the commit contains only Task 1 files.

---

### Task 2: Expose semantic children from custom widgets

**Repository:** The same Makepad worktree

**Files:**
- Modify: `widgets/src/widget.rs:211-340`
- Modify: `widgets/src/widget_tree.rs:1-12`
- Modify: `widgets/src/widget_tree.rs:1955-2175`
- Modify: `widgets/src/widget_tree.rs` test module
- Modify: `libs/makepad_test/GUIDE.md`

**Interfaces:**
- Produces: `makepad_widgets::WidgetSemanticItem`.
- Produces: object-safe `fn semantic_items(&self, cx: &Cx) -> Vec<WidgetSemanticItem>` on `Widget`, with a default empty result.
- Consumes: `WidgetSemanticItem.rect` in owning-window client coordinates.
- Produces: one ordinary `WidgetSnapshot` per emitted semantic item, with the owning widget's window identity and window-position offset.
- Preserves: existing snapshots for ordinary widgets and Dock virtual items.

- [ ] **Step 1: Write the failing semantic-item snapshot test**

Define this API in the test code before implementation use:

```rust
impl Widget for SemanticTestWidget {
    fn semantic_items(&self, _cx: &Cx) -> Vec<WidgetSemanticItem> {
        vec![WidgetSemanticItem {
            id: "row:orders".into(),
            widget_type: "TestTreeRow".into(),
            rect: Rect {
                pos: dvec2(10.0, 20.0),
                size: dvec2(80.0, 24.0),
            },
            visible: true,
            enabled: true,
            text: Some("Orders".into()),
            value: Some("orders".into()),
            checked: Some(true),
            selected: Some("orders".into()),
        }]
    }
}
```

Add a `widget_tree_snapshot_includes_custom_semantic_items` test. Register a
`SemanticTestWidget` as the tree root, call `WidgetTree::snapshot`, find
`id == "row:orders"`, and assert every field and rectangle value.

- [ ] **Step 2: Run the test and confirm the red state**

Run:

```powershell
rtk cargo test -p makepad-widgets widget_tree_snapshot_includes_custom_semantic_items
```

Expected: compilation fails because `WidgetSemanticItem` and
`Widget::semantic_items` do not exist.

- [ ] **Step 3: Define the semantic-item value and default hook**

Add to `widgets/src/widget.rs`:

```rust
#[derive(Clone, Debug, Default, PartialEq)]
pub struct WidgetSemanticItem {
    pub id: String,
    pub widget_type: String,
    pub rect: Rect,
    pub visible: bool,
    pub enabled: bool,
    pub text: Option<String>,
    pub value: Option<String>,
    pub checked: Option<bool>,
    pub selected: Option<String>,
}
```

Add to `Widget`:

```rust
/// Return actionable semantic children drawn inside this widget.
///
/// Rectangles use owning-window client coordinates. The widget-tree snapshot
/// collector adds the window position and identity.
fn semantic_items(&self, _cx: &Cx) -> Vec<WidgetSemanticItem> {
    Vec::new()
}
```

Add a `WidgetRef::semantic_items(&self, cx: &Cx)` forwarding method that returns
an empty vector when the widget is empty or already borrowed.

- [ ] **Step 4: Merge semantic items into snapshots**

After the ordinary widget snapshot and before Dock virtual entries, append each
custom item as a `WidgetSnapshot`. Copy semantic fields unchanged. Use the
resolved window ID/index. Add the window position to `rect.pos`, then round
position and size using the same rules as ordinary widgets.

Do not add semantic items to `query_rects`; `makepad-test::Locator` resolves and
clicks from structured snapshots.

- [ ] **Step 5: Run focused and package verification**

Run:

```powershell
rtk cargo test -p makepad-widgets widget_tree_snapshot_includes_custom_semantic_items
rtk cargo test -p makepad-widgets widget_tree
rtk cargo test -p makepad-test
```

Expected: all commands pass and existing snapshot/query behavior is unchanged.

- [ ] **Step 6: Document custom-drawn controls**

Add a `Custom-drawn controls` section to `libs/makepad_test/GUIDE.md`. State:

- Emit one `WidgetSemanticItem` per actionable or observable virtual child.
- Use stable semantic identity and type names.
- Use the same rectangles as hit testing.
- Do not expose paint-only fragments.
- Keep application scenarios independent of item IDs through an adapter.

- [ ] **Step 7: Format, verify, and commit**

Run:

```powershell
rtk cargo fmt --all -- --check
rtk cargo test -p makepad-widgets widget_tree
rtk cargo test -p makepad-test
rtk git diff --check
rtk git add widgets/src/widget.rs widgets/src/widget_tree.rs libs/makepad_test/GUIDE.md
rtk git commit -m "feat(widgets): expose semantic child items"
rtk git rev-parse HEAD
```

Expected: the final command prints the Makepad commit that contains Tasks 1
and 2.

## External Makepad Integration Gate

Before Task 3:

1. Review the two Makepad commits as one branch against `origin/waml`.
2. Run `rtk cargo test -p makepad-test` and
   `rtk cargo test -p makepad-widgets widget_tree` again after any rebase.
3. Ask the user for approval to push the Makepad branch.
4. Push the branch to `origin`.
5. Verify that `rtk git ls-remote origin` contains the Task 2 HEAD commit.

Task 3 must not commit a WAML Git dependency that points to an unreachable
Makepad object.

---

### Task 3: Add the typed WAML UI-test packages and macro

**Repository:** WAML worktree

**Files:**
- Modify: `Cargo.toml:1-2`
- Modify: `Cargo.toml` Makepad `unicode-bidi` revision
- Modify: `Cargo.lock`
- Create: `crates/waml-ui-test-macros/Cargo.toml`
- Create: `crates/waml-ui-test-macros/src/lib.rs`
- Create: `crates/waml-ui-test/Cargo.toml`
- Create: `crates/waml-ui-test/src/lib.rs`
- Create: `crates/waml-ui-test/src/config.rs`

**Interfaces:**
- Consumes: the reachable Makepad HEAD produced by Task 2.
- Produces: `#[waml_ui_test(workspace = Mini)]`.
- Produces: `WorkspaceFixture::Mini` and `ScenarioConfig`.
- Produces: hidden `waml_ui_test::__private::run_catalog_test` called by macro expansion.
- Defers: fixture staging and real application launch to Task 4.

- [ ] **Step 1: Capture and validate the reachable Makepad revision**

From the Makepad worktree, run:

```powershell
$makepadUiTestSha = (rtk git rev-parse HEAD).Trim()
$remoteRefs = rtk git ls-remote origin
if (-not ($remoteRefs -match "^$makepadUiTestSha\\s")) {
    throw "Makepad UI-test commit is not reachable from origin"
}
```

Do not proceed until the exact reviewed commit appears in a remote branch ref.
The branch starts from `origin/waml`, but the feature commit does not need to be
merged into `origin/waml` before WAML pins it.

Replace the `makepad-widgets` and workspace `unicode-bidi` `rev` values with
the exact value in `$makepadUiTestSha`. The support crate's `makepad-test`
dependency must use the same Git URL and revision.

- [ ] **Step 2: Write failing proc-macro expansion tests**

Create macro tests for:

```rust
#[waml_ui_test(workspace = Mini)]
fn navigation(mut app: WamlApp) {
    app.expect_workspace_open();
}
```

Assert that expansion:

- Creates a normal `#[test]` wrapper.
- Preserves `#[ignore]` and `#[should_panic]` on the wrapper only.
- Calls `::waml_ui_test::__private::run_catalog_test`.
- Passes `env!("CARGO_MANIFEST_DIR")`, `env!("CARGO_PKG_NAME")`,
  `module_path!()`, the test name, and `WorkspaceFixture::Mini`.
- Rejects missing `workspace`, unknown keys, async functions, generic
  functions, methods, and signatures other than one identifier argument.

- [ ] **Step 3: Run the macro crate test and confirm the red state**

Run:

```powershell
rtk cargo test -p waml-ui-test-macros
```

Expected: Cargo reports that the new package or its implementation is absent.

- [ ] **Step 4: Create the proc-macro crate**

Use:

```toml
[package]
name = "waml-ui-test-macros"
version = "0.0.0"
edition.workspace = true
rust-version.workspace = true
license.workspace = true
publish = false

[lib]
proc-macro = true

[dependencies]
proc-macro2 = "1"
quote = "1"
syn = { version = "2", features = ["full"] }
```

Follow the existing `makepad-test-macros` expansion structure. Parse exactly
one `workspace = <Ident>` argument and generate a private inner function plus a
normal test wrapper.

- [ ] **Step 5: Create the support crate catalog and public exports**

Use this support-crate manifest, substituting the exact reachable revision from
Step 1 in both Makepad dependencies:

```toml
[package]
name = "waml-ui-test"
version = "0.0.0"
edition.workspace = true
rust-version.workspace = true
license.workspace = true
publish = false

[dependencies]
makepad-test = { git = "https://github.com/redoz/makepad.git", rev = "<reachable-sha>" }
makepad-widgets = { git = "https://github.com/redoz/makepad.git", rev = "<reachable-sha>" }
serde.workspace = true
serde_json.workspace = true
waml-ui-test-macros = { path = "../waml-ui-test-macros" }

[dev-dependencies]
tempfile.workspace = true
```

`<reachable-sha>` is an instruction-time substitution marker. It must not
remain in a committed manifest.

Create `WorkspaceFixture` and `ScenarioConfig`:

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkspaceFixture {
    Mini,
}

pub struct ScenarioConfig {
    pub package_name: &'static str,
    pub manifest_dir: &'static str,
    pub module_path: &'static str,
    pub test_name: &'static str,
    pub workspace: WorkspaceFixture,
}
```

Add crate-private fixture metadata for `Mini`:

```rust
pub(crate) struct FixtureDescriptor {
    pub relative_path: &'static str,
    pub ready_diagram: &'static str,
}
```

Map `Mini` to `tests/fixtures/mini` and ready diagram `Orders`.

Export the macro, `WorkspaceFixture`, and a temporary zero-sized `WamlApp`.
Implement hidden `run_catalog_test` as a unit-test-safe stub that constructs
`ScenarioConfig` and calls the supplied function. Task 4 replaces only the
stub body, not its signature.

- [ ] **Step 6: Register the support packages**

Add both support crates to workspace members. Do not register the editor UI
test target until Task 6 creates `tests/ui.rs`; Cargo must never point to a
source file that does not exist.

- [ ] **Step 7: Run focused tests and workspace compilation**

Run:

```powershell
rtk cargo test -p waml-ui-test-macros
rtk cargo test -p waml-ui-test
rtk cargo test -p waml-editor --lib
```

Expected: macro tests pass, the support crate catalog tests pass, and existing
editor unit tests remain green.

- [ ] **Step 8: Format, verify, and commit**

Run:

```powershell
rtk cargo fmt --all -- --check
rtk git diff --check
rtk git add Cargo.toml Cargo.lock crates/waml-editor/Cargo.toml crates/waml-ui-test crates/waml-ui-test-macros
rtk git commit -m "build(test): add WAML UI test support crates"
```

Expected: the commit contains the exact remote Makepad pin plus compiling
support packages and macro tests.

---

### Task 4: Implement isolated launch, semantic tracing, and failure ownership

**Files:**
- Modify: `crates/waml-ui-test/src/lib.rs`
- Modify: `crates/waml-ui-test/src/config.rs`
- Create: `crates/waml-ui-test/src/fixture.rs`
- Create: `crates/waml-ui-test/src/trace.rs`
- Create: `crates/waml-ui-test/src/error.rs`
- Create: `crates/waml-ui-test/src/run.rs`
- Create: `crates/waml-ui-test/src/app.rs`
- Create: `crates/waml-ui-test/src/domain.rs`

**Interfaces:**
- Produces: `pub struct WamlApp` wrapping one `makepad_test::TestApp`.
- Produces: `pub const DiagramName::ORDERS` and `pub enum ViewKind { Diagram, Source }`.
- Produces: private `fn run_scenario(config: ScenarioConfig, scenario: impl FnOnce(WamlApp))`.
- Produces: unique `RunIdentity { run_id, test_slug, title, run_root }`.
- Produces: `SemanticTrace::begin/pass/fail` with immediate text and JSON persistence.
- Produces: one cleanup path for catalog workspaces; failed workspaces remain.

- [ ] **Step 1: Write failing run-identity and title tests**

Test that two calls for the same test name produce different run roots and
titles. Test that the title:

- Uses only lowercase ASCII letters, digits, and hyphens.
- Starts with `ui-`.
- Is no longer than 48 bytes.
- Contains the process ID/counter identity and a truncated test slug.

Also test that module paths and punctuation cannot create path separators.

- [ ] **Step 2: Write failing fixture staging tests**

Use `tempfile::TempDir` to create a synthetic workspace root with
`crates/waml-editor/tests/fixtures/mini/index.md` and `orders.md`.

Test that `stage_fixture`:

- Copies both files byte-for-byte into the run-owned workspace.
- Does not change the source after changing the staged copy.
- Rejects symlinks/reparse points.
- Refuses to remove the ownership root itself.
- Refuses any cleanup candidate outside the ownership root.
- Removes a successful run root.
- Preserves a failed run root.

- [ ] **Step 3: Write failing semantic trace tests**

Use this record model:

```rust
#[derive(Clone, Debug, serde::Serialize)]
pub struct StepRecord {
    pub sequence: u32,
    pub operation: String,
    pub expected: String,
    pub observed: String,
    pub outcome: StepOutcome,
}

#[derive(Clone, Debug, serde::Serialize)]
pub enum StepOutcome {
    Running,
    Passed,
    Failed { detail: String },
}
```

Test that `begin` immediately writes a running record to
`semantic-trace.txt` and `semantic-trace.json`, and that `pass`/`fail` replaces
the outcome and persists the observed state.

- [ ] **Step 4: Run the support tests and confirm the red state**

Run:

```powershell
rtk cargo test -p waml-ui-test
```

Expected: tests fail because run identity, staging, and trace modules are absent.

- [ ] **Step 5: Implement safe staging and unique ownership**

Resolve the WAML workspace root as the parent of the `crates` directory that
contains the `waml-editor` manifest. Build all destination components from
sanitized internal values. Before `remove_dir_all`, require:

```rust
candidate.starts_with(ownership_root) && candidate != ownership_root
```

Reject source symlinks instead of following them. Copy regular files and
directories recursively. Do not copy file timestamps or permissions that are
not needed by the fixture.

- [ ] **Step 6: Implement the persisted trace and errors**

Use:

```rust
pub(crate) struct OperationFailure {
    pub observed: String,
    pub detail: String,
}

pub struct WamlUiError {
    pub test_name: String,
    pub sequence: u32,
    pub operation: String,
    pub expected: String,
    pub observed: String,
    pub detail: String,
    pub artifacts_dir: PathBuf,
}
```

`Display` must follow the specification's semantic error format. It must not
print only a selector or driver error.

- [ ] **Step 7: Implement one launch lifecycle**

`run_catalog_test` builds `ScenarioConfig` and calls `run_scenario`.
`run_scenario` must:

1. Allocate `RunIdentity`.
2. Stage the fixture.
3. Create `makepad_test::TestConfig::current_package`.
4. Override `artifacts_dir` with the run-owned directory.
5. Set `args` to staged workspace path, `--title`, and the unique title.
6. Call `makepad_test::run_with_config`.
7. Construct `WamlApp` with the authoritative workspace binding and trace.
8. Remove the run root only on success.
9. Preserve the run root on error or panic and propagate the semantic failure.

Do not add a global mutex. `makepad-test` can keep its current exclusive
session lease below this boundary.

- [ ] **Step 8: Implement the semantic execution envelope**

Add:

```rust
impl WamlApp {
    pub(crate) fn execute(
        &mut self,
        operation: impl Into<String>,
        expected: impl Into<String>,
        action: impl FnOnce(&makepad_test::TestApp) -> Result<String, OperationFailure>,
    ) -> &mut Self;
}
```

The method must begin and persist the trace before the action. On success, it
records `Passed`. On failure, it records `Failed`, constructs `WamlUiError`,
and panics with its display text so `makepad-test` captures its normal failure
artifacts. The action closure can perform zero or more interactions.

- [ ] **Step 9: Run focused tests and commit**

Run:

```powershell
rtk cargo test -p waml-ui-test
rtk cargo test -p waml-ui-test-macros
rtk cargo fmt --all -- --check
rtk git diff --check
rtk git add crates/waml-ui-test
rtk git commit -m "test(ui): add semantic scenario harness"
```

Expected: lifecycle, safety, trace, and error tests pass without launching the
editor.

---

### Task 5: Expose project-tree rows as semantic controls

**Files:**
- Modify: `crates/waml-editor/src/tree_layout.rs:65-81`
- Modify: `crates/waml-editor/src/tree_layout.rs:172-212`
- Modify: `crates/waml-editor/src/tree_panel.rs:539-810`
- Modify: `crates/waml-editor/src/tree_panel.rs` test module

**Interfaces:**
- Produces: `TreeLayout::viewport_rect(&self) -> Rect`.
- Produces: one `WidgetSemanticItem` with type `WamlProjectTreeRow` for each logical row.
- Uses: `id = format!("project-tree-row:{}", row.key)`.
- Uses: `text = Some(row.title.clone())`.
- Uses: `value = row.concept_id.clone().or_else(|| row.address.clone())`.
- Uses: `checked = Some(layout.selected() == Some(row.key.as_str()))`.
- Uses: `selected = checked.then(|| row.key.clone())`.
- Uses: the same clipped row rectangle as pointer hit testing.

- [ ] **Step 1: Write failing viewport and semantic-row tests**

Add a pure `TreeLayout::viewport_rect` test after `set_viewport`.

Add `project_tree_semantic_items_identify_visible_openable_rows` with a
`TreeLayout` containing the `mini`-style Orders diagram. Set a viewport, select
Orders, then assert:

```rust
let orders_row_key = layout.selected().expect("Orders row selected");
assert_eq!(orders.widget_type, "WamlProjectTreeRow");
assert_eq!(orders.text.as_deref(), Some("Orders"));
assert_eq!(orders.value.as_deref(), Some("orders"));
assert_eq!(orders.checked, Some(true));
assert_eq!(orders.selected.as_deref(), Some(orders_row_key));
assert!(orders.enabled);
assert!(orders.visible);
```

Also test that an off-viewport row is present with `visible == false`, a
non-openable row has `enabled == false`, and a partially clipped row's semantic
rectangle stays inside the viewport.

- [ ] **Step 2: Run the focused tests and confirm the red state**

Run:

```powershell
rtk cargo test -p waml-editor viewport_rect_reports_drawn_tree_body
rtk cargo test -p waml-editor project_tree_semantic_items_identify_visible_openable_rows
```

Expected: compilation fails because the accessor and semantic-item helper do
not exist.

- [ ] **Step 3: Add viewport access and rectangle clipping**

Implement:

```rust
pub fn viewport_rect(&self) -> Rect {
    Rect {
        pos: self.origin,
        size: self.size,
    }
}
```

Add a private rectangle-intersection helper in `tree_panel.rs`. It returns
`None` for an empty intersection and never moves a row outside the viewport.

- [ ] **Step 4: Emit semantic items from `ProjectTree`**

Implement `Widget::semantic_items` by calling a pure private helper with
`&self.layout` and `self.presentation_visible`. Emit logical rows even when
off-screen, but mark them invisible and give them their unclipped row rectangle.
For a visible row, use the clipped rectangle. The helper must not duplicate
the row model or calculate a second layout.

- [ ] **Step 5: Run editor verification**

Run:

```powershell
rtk cargo test -p waml-editor tree_layout
rtk cargo test -p waml-editor tree_panel
rtk cargo test -p waml-editor --lib
```

Expected: all existing and new tree tests pass.

- [ ] **Step 6: Format, verify, and commit**

Run:

```powershell
rtk cargo fmt --all -- --check
rtk git diff --check
rtk git add crates/waml-editor/src/tree_layout.rs crates/waml-editor/src/tree_panel.rs
rtk git commit -m "feat(editor): expose project tree semantics"
```

Expected: the commit changes only the ProjectTree semantic contract and its
tests.

---

### Task 6: Implement navigation adapters and the first semantic journey

**Files:**
- Modify: `crates/waml-editor/Cargo.toml`
- Modify: `crates/waml-ui-test/src/lib.rs`
- Modify: `crates/waml-ui-test/src/app.rs`
- Modify: `crates/waml-ui-test/src/domain.rs`
- Create: `crates/waml-ui-test/src/adapters/mod.rs`
- Create: `crates/waml-ui-test/src/adapters/workspace.rs`
- Create: `crates/waml-ui-test/src/adapters/documents.rs`
- Create: `crates/waml-editor/tests/ui.rs`

**Interfaces:**
- Produces: `DiagramName::ORDERS` with display text `Orders` and semantic value `orders`.
- Produces: `pub const fn DiagramName::new(display: &'static str, value: &'static str) -> Self` for typed catalog extension and controlled failure probes.
- Produces: `ViewKind::{Diagram, Source}`.
- Produces: public chainable methods:
  - `WamlApp::expect_workspace_open(&mut self) -> &mut Self`
  - `WamlApp::ensure_diagram_open(&mut self, DiagramName) -> &mut Self`
  - `WamlApp::expect_active_diagram(&mut self, DiagramName) -> &mut Self`
  - `WamlApp::switch_active_document_to(&mut self, ViewKind) -> &mut Self`
  - `WamlApp::expect_active_view(&mut self, ViewKind) -> &mut Self`
- Keeps private: `Selector::widget_type("WamlProjectTreeRow")`, `view_button`, `canvas_wrap`, and `markdown_surface`.

- [ ] **Step 1: Write failing adapter observation tests**

Build ordinary `makepad_test::WidgetSnapshot` values in unit tests. Cover:

- Mini is ready when a visible enabled `WamlProjectTreeRow` named Orders exists.
- A duplicate Orders row is an error, not an arbitrary first match.
- Orders is active only when its row has `checked == Some(true)` and
  `canvas_wrap` is visible.
- Diagram view means visible `canvas_wrap` and hidden `markdown_surface`.
- Source view means hidden `canvas_wrap` and visible `markdown_surface`.
- Both visible or both hidden is an invalid observed state.

- [ ] **Step 2: Register the gated target and write the first scenario**

Add `waml-ui-test` as a `waml-editor` dev-dependency, then add:

```toml
[features]
ui-tests = []

[[test]]
name = "ui"
path = "tests/ui.rs"
required-features = ["ui-tests"]
```

Create `crates/waml-editor/tests/ui.rs` with only semantic imports and calls:

```rust
use waml_ui_test::{waml_ui_test, DiagramName, ViewKind, WamlApp};

#[waml_ui_test(workspace = Mini)]
fn open_and_switch_document_views(mut app: WamlApp) {
    app.expect_workspace_open()
        .ensure_diagram_open(DiagramName::ORDERS)
        .expect_active_diagram(DiagramName::ORDERS)
        .switch_active_document_to(ViewKind::Source)
        .expect_active_view(ViewKind::Source)
        .switch_active_document_to(ViewKind::Diagram)
        .expect_active_view(ViewKind::Diagram);
}
```

- [ ] **Step 3: Run compilation and confirm the red state**

Run:

```powershell
rtk cargo test -p waml-ui-test
rtk cargo test -p waml-editor --features ui-tests --test ui --no-run
```

Expected: compilation fails because adapter functions and `WamlApp` methods do
not exist.

- [ ] **Step 4: Implement private selectors and observations**

In `workspace.rs`, expose only crate-private functions. Use:

```rust
Selector::widget_type("WamlProjectTreeRow").text_exact("Orders")
```

In `documents.rs`, use the same typed row selector plus private selectors for:

```rust
Selector::id("view_button")
Selector::id("canvas_wrap")
Selector::id("markdown_surface")
```

Use `try_*` Locator methods. Never call the panicking convenience methods from
an adapter. Convert driver failures to `OperationFailure` with a semantic
observed-state description.

- [ ] **Step 5: Implement each `WamlApp` operation through `execute`**

`expect_workspace_open` observes only.

`ensure_diagram_open`:

1. Return success without input if Orders is already active.
2. Resolve exactly one visible enabled Orders row.
3. Click it.
4. Wait for its checked state and Diagram view.

`expect_active_diagram` observes only.

`switch_active_document_to`:

1. Fail when the requested view is already active; this is an action, not an
   `ensure_*` operation.
2. Click the one visible `view_button`.
3. Wait for the requested surface state.

`expect_active_view` observes only.

Each method must supply an operation name and expected semantic state to the
shared execution envelope.

- [ ] **Step 6: Run unit and compile verification**

Run:

```powershell
rtk cargo test -p waml-ui-test
rtk cargo test -p waml-editor --features ui-tests --test ui --no-run
rtk cargo test -p waml-editor --lib
```

Expected on Windows: unit tests and UI target compilation pass. Do not run the
headless UI binary on Windows.

- [ ] **Step 7: Qualify the journey on Linux**

On the same Ubuntu image and package set used by `.github/workflows/ci.yml`, run:

```bash
rtk cargo test -p waml-editor --features ui-tests --test ui -- --test-threads=1
```

Expected: `open_and_switch_document_views` passes. If the build fails in
generic Makepad headless code, fix that defect in the Makepad worktree, repeat
Tasks 1-2 verification, publish the new reviewed commit, and update both WAML
Makepad pins before continuing. Do not add a WAML-only backend workaround.

- [ ] **Step 8: Run the same scenario visibly for diagnosis parity**

Start Makepad Studio manually. Use one persistent Studio remote bridge, clear
an older `waml-editor` build, and run:

```powershell
$env:MAKEPAD_TEST_VISIBLE='1'
$env:MAKEPAD_TEST_STUDIO='127.0.0.1:8001'
$env:MAKEPAD_TEST_STUDIO_MOUNT='waml'
rtk cargo test -p waml-editor --features ui-tests --test ui -- --test-threads=1
```

Expected: the test passes without source changes and the unique title starts
with `ui-`.

- [ ] **Step 9: Format, verify, and commit**

Run:

```powershell
rtk cargo fmt --all -- --check
rtk cargo test -p waml-ui-test
rtk cargo test -p waml-editor --features ui-tests --test ui --no-run
rtk git diff --check
rtk git add crates/waml-ui-test crates/waml-editor/tests/ui.rs
rtk git commit -m "test(ui): cover document view navigation"
```

Expected: scenario code contains no Makepad implementation details.

---

### Task 7: Gate Linux UI navigation and update verification ownership

**Files:**
- Modify: `.github/workflows/ci.yml`
- Modify: `crates/waml-editor/tests/README.md`

**Interfaces:**
- Produces: one Linux-only required UI-test step after the normal workspace test.
- Preserves: normal `cargo nextest run --workspace --profile ci` on Windows and Linux.
- Preserves: Windows compilation through `cargo clippy --all-targets --all-features`.
- Removes: automated navigation steps from the manual verification-of-record list.

- [ ] **Step 1: Prove the normal suite does not run the UI target**

Run:

```powershell
rtk cargo nextest run --workspace --profile ci
```

Expected: the normal suite passes and does not launch
`open_and_switch_document_views`, because the integration target requires the
`ui-tests` feature.

- [ ] **Step 2: Add the Linux-only required step**

After `Cargo test`, add:

```yaml
      - name: Semantic editor UI test
        if: runner.os == 'Linux'
        run: cargo test -p waml-editor --features ui-tests --test ui -- --test-threads=1
```

Keep this step inside the existing required `build-test` job. Do not use
`continue-on-error`.

- [ ] **Step 3: Update editor test documentation**

In `crates/waml-editor/tests/README.md`:

- Add the headless semantic test command.
- Add the visible Studio command and required environment variables.
- Explain the DSL rule and failure artifact directory.
- State that Linux has automated semantic coverage while Windows headless
  remains unavailable.
- Remove fixture load, Orders diagram activation, and Diagram/Source switching
  from verification-of-record manual checks.
- Keep visual rendering, temporal canvas gestures, and uncovered navigation as
  manual checks.

- [ ] **Step 4: Verify the failure artifact contract with a controlled red probe**

In the WAML worktree only, temporarily change the scenario's
`DiagramName::ORDERS` argument to a new local `DiagramName::new("Missing",
"missing")`. Run the Linux UI command and require failure.

Confirm the printed artifact directory contains:

```text
semantic-trace.txt
semantic-trace.json
failure.txt
logs.txt
widget-snapshot.json
widget-tree.txt
failure-screenshot.png
workspace/
```

Restore the scenario with `apply_patch`, rerun the UI command, and require a
pass. Do not use `git checkout --` or another destructive restore command.

- [ ] **Step 5: Run final WAML verification**

Run:

```powershell
rtk cargo fmt --all -- --check
rtk cargo test -p waml-ui-test-macros
rtk cargo test -p waml-ui-test
rtk cargo test -p waml-editor --lib
rtk cargo test -p waml-editor --features ui-tests --test ui --no-run
rtk cargo nextest run --workspace --profile ci
rtk cargo clippy --workspace --all-targets --all-features -- -D warnings
rtk git diff --check
```

On Linux, additionally run the real semantic UI command. Expected: all checks
pass, with the UI journey executed exactly once in its dedicated command.

- [ ] **Step 6: Commit the CI and ownership update**

Run:

```powershell
rtk git add .github/workflows/ci.yml crates/waml-editor/tests/README.md
rtk git commit -m "ci(ui): gate semantic navigation test"
rtk git status --short
```

Expected: the worktree is clean. The WAML branch contains five focused commits
after the design/plan commits, and the Makepad branch contains two focused
foundation commits.

---

## Final Review Checklist

- [ ] Makepad `TestConfig::args` defaults empty and reaches both launch modes.
- [ ] Makepad custom semantics are generic and have no WAML-specific branch.
- [ ] The Makepad commit pinned by WAML is reachable from `origin`.
- [ ] The macro supplies the only workspace identity.
- [ ] Catalog staging, tracing, cleanup, and evidence use one lifecycle.
- [ ] Run ownership is unique even though the first CI policy is serial.
- [ ] `ProjectTree` semantic rectangles come from `TreeLayout::row_rect` and the
      current viewport, not a second layout.
- [ ] Scenario code imports only `waml_ui_test` semantic types.
- [ ] Assertions observe semantic postconditions instead of attempted clicks.
- [ ] The normal nextest suite does not launch UI tests.
- [ ] Linux runs the dedicated semantic UI test as a required check.
- [ ] Windows compiles all feature-gated support code without running headless.
- [ ] A controlled failure preserves all specified artifacts and its workspace.
- [ ] The same scenario source passes in headless and visible Studio modes.
