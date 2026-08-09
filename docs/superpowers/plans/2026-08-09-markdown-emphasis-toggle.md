# Markdown Emphasis Toggle Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let each source or Markdown-editor tab switch independently between `Code` and `Layout` emphasis, with a per-user configuration value copied into each new tab.

**Architecture:** `EditorConfig` deserializes a lower-case configuration value and exposes it as `waml_markdown_editor::EditorEmphasis`. `App` loads that value into session-owned state and passes a copy through the existing document factories into each `SourceView`. `SourceView` owns the live value, applies it through `MarkdownEditorRef::set_emphasis`, and uses the existing single destination-state header action.

**Tech Stack:** Rust, Serde/JSON, Makepad widgets, `waml-markdown-editor`, Cargo unit tests.

## Global Constraints

- Use ASD-STE100 Simplified Technical English.
- Work only in `C:\dev\waml\.worktrees\markdown-emphasis-toggle`.
- Use TDD. Confirm the expected RED result before each production change.
- The per-user editor configuration owns the default. `.waml/settings.json` must not contain it.
- An absent `markdown_emphasis` defaults to `Code`.
- Invalid values use the existing configuration-load failure path.
- A tab action changes only that tab. It must not store configuration or change another tab.
- Keep one header action button. It shows the destination mode and its tooltip.
- Apply emphasis only through `MarkdownEditorRef::set_emphasis`.
- Do not add settings UI or per-tab persistence.
- Prefix shell commands with `rtk`.

---

### Task 1: Add the user configuration default

**Files:**

- Modify: `crates/waml-editor/src/config.rs:107-127`
- Test: `crates/waml-editor/src/config.rs:563-631`

**Interfaces:**

- Consumes: `waml_markdown_editor::EditorEmphasis::{Code, Layout}`.
- Produces: `pub fn markdown_emphasis() -> EditorEmphasis`.
- Produces JSON field: `markdown_emphasis: "code" | "layout"`.

- [ ] **Step 1: Write the failing compatibility and round-trip tests**

Add focused tests to the existing `config.rs` test module:

```rust
#[test]
fn markdown_emphasis_field_absent_in_old_file_loads_code() {
    let tmp = TempDir::new();
    std::fs::write(
        tmp.path().join(EDITOR_FILE),
        br#"{"version":1,"recents":[],"theme":"light"}"#,
    )
    .unwrap();

    let cfg: EditorConfig = load_from(tmp.path(), EDITOR_FILE);

    assert_eq!(cfg.markdown_emphasis, MarkdownEmphasis::Code);
}

#[test]
fn markdown_emphasis_code_and_layout_round_trip() {
    for emphasis in [MarkdownEmphasis::Code, MarkdownEmphasis::Layout] {
        let tmp = TempDir::new();
        let cfg = EditorConfig {
            version: EDITOR_VERSION,
            recents: Vec::new(),
            theme: ThemeMode::Light,
            markdown_emphasis: emphasis,
        };

        store_to(tmp.path(), EDITOR_FILE, &cfg).unwrap();
        let back: EditorConfig = load_from(tmp.path(), EDITOR_FILE);

        assert_eq!(back.markdown_emphasis, emphasis);
    }
}
```

- [ ] **Step 2: Run the tests and confirm RED**

Run:

```bash
rtk cargo test -p waml-editor markdown_emphasis -- --nocapture
```

Expected: compilation fails because `MarkdownEmphasis` and `EditorConfig::markdown_emphasis` do not exist.

- [ ] **Step 3: Add the schema type, field, conversion, and accessor**

```rust
use waml_markdown_editor::EditorEmphasis;

#[derive(Serialize, Deserialize, Clone, Copy, Default, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
enum MarkdownEmphasis {
    #[default]
    Code,
    Layout,
}

impl From<MarkdownEmphasis> for EditorEmphasis {
    fn from(value: MarkdownEmphasis) -> Self {
        match value {
            MarkdownEmphasis::Code => EditorEmphasis::Code,
            MarkdownEmphasis::Layout => EditorEmphasis::Layout,
        }
    }
}
```

Add `#[serde(default)] markdown_emphasis: MarkdownEmphasis` to `EditorConfig`, update its existing test literals, and add:

```rust
pub fn markdown_emphasis() -> EditorEmphasis {
    let config: EditorConfig = load(EDITOR_FILE);
    config.markdown_emphasis.into()
}
```

Do not add a setter.

- [ ] **Step 4: Run the configuration tests**

```bash
rtk cargo test -p waml-editor config::tests -- --nocapture
```

Expected: all configuration tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/waml-editor/src/config.rs
git commit -m "feat(editor): configure markdown emphasis"
```

### Task 2: Give the header button a typed destination

**Files:**

- Modify: `crates/waml-editor/src/doc_view.rs:250-273,564-595`
- Modify: `crates/waml-editor/src/document_header.rs:220-294,438-475`
- Modify: `crates/waml-editor/src/icon_button.rs:84-188,226-364`
- Modify: `crates/waml-editor/src/app.rs:628-633`
- Modify: `crates/waml-editor/src/app/shell.rs:848-880`
- Modify: `crates/waml-editor/src/source_toggle_view.rs:190-208,291-327`
- Update `DocumentHeaderChrome` literals in the files reported by the compiler.

**Interfaces:**

- Consumes: the existing `view_button`, `IconButton::set_icon`, and tooltip overlay pattern.
- Produces:

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HeaderViewAction {
    pub icon: Icon,
    pub tooltip: &'static str,
}
```

- Changes `DocumentHeaderChrome::view_toggle` from `Option<Icon>` to `Option<HeaderViewAction>`.
- Changes `DocumentHeader::set_view_toggle` to accept `Option<HeaderViewAction>`.
- Produces `IconButton::set_tooltip(Option<&str>)` and the matching `IconButtonRef` forwarder.

- [ ] **Step 1: Write failing destination-state tests**

In `source_toggle_view.rs`, extend the existing chrome tests:

```rust
assert_eq!(
    view.chrome().document_header.view_toggle,
    Some(HeaderViewAction {
        icon: Icon::Code,
        tooltip: "View source",
    })
);

view.toggle_for_test();

assert_eq!(
    view.chrome().document_header.view_toggle,
    Some(HeaderViewAction {
        icon: Icon::Eye,
        tooltip: "View rendered",
    })
);
```

In `document_header.rs`, add a state test that replaces one `HeaderViewAction`, preserves its icon and tooltip, and reserves exactly one trailing-button width.

- [ ] **Step 2: Run the focused tests and confirm RED**

```bash
rtk cargo test -p waml-editor source_toggle_view::tests -- --nocapture
rtk cargo test -p waml-editor document_header::tests -- --nocapture
```

Expected: compilation fails because `HeaderViewAction` does not exist and `view_toggle` still stores `Option<Icon>`.

- [ ] **Step 3: Add the typed action and pass it through header projection**

Add `HeaderViewAction` beside `DocumentHeaderChrome`. Update `BodyWidgets::apply_chrome`, `project_document_header`, `App::sync_document_shell`, `DocumentHeaderState`, and `DocumentHeader::set_view_toggle` to pass the value intact.

```rust
pub fn set_view_toggle(
    &mut self,
    cx: &mut Cx,
    action: Option<HeaderViewAction>,
) {
    if !self.state.replace_view_toggle(action) {
        return;
    }

    let button = self.view.widget(cx, ids!(view_button));
    button.set_visible(cx, action.is_some());
    if let Some(action) = action {
        button.as_icon_button().set_icon(cx, action.icon);
        button.as_icon_button().set_tooltip(Some(action.tooltip));
    } else {
        button.as_icon_button().set_tooltip(None);
    }
    self.sync_content_layout(cx);
}
```

Keep the existing source/rendered control as one destination button with `View source` and `View rendered` tooltips.

- [ ] **Step 4: Connect `IconButton` to the existing tooltip action pattern**

Store `tooltip: Option<String>` on `IconButton`. On `FingerHoverIn`, emit the repository's existing `TooltipAction::HoverIn` with the configured text and button rectangle. On `FingerHoverOut`, emit `TooltipAction::HoverOut`. Buttons without text emit no tooltip action. Reuse the application tooltip overlay; add one only if the application tree does not already contain it.

- [ ] **Step 5: Verify and commit**

```bash
rtk cargo test -p waml-editor document_header::tests -- --nocapture
rtk cargo test -p waml-editor source_toggle_view::tests -- --nocapture
rtk cargo test -p waml-editor doc_view::tests -- --nocapture
git add crates/waml-editor/src
git commit -m "feat(editor): describe header destinations"
```

### Task 3: Copy the session default into every new source view

**Files:**

- Modify: `crates/waml-editor/src/app.rs:640-670,801-839`
- Modify: `crates/waml-editor/src/app/navigation.rs:480-625`
- Modify: `crates/waml-editor/src/app/workspace.rs:151-170`
- Modify: `crates/waml-editor/src/documents.rs:36-285`
- Modify: `crates/waml-editor/src/okf_documents.rs:40-220`
- Modify: `crates/waml-editor/src/uml_documents.rs:79-125`
- Modify: `crates/waml-editor/src/generic_okf_view.rs:23-47`
- Modify: `crates/waml-editor/src/source_toggle_view.rs:20-40`
- Modify: `crates/waml-editor/src/source_view.rs:5-18,107-142,236-369`

**Interfaces:**

- Consumes: `config::markdown_emphasis() -> EditorEmphasis`.
- Produces: `App::markdown_emphasis: EditorEmphasis`.
- Adds an `EditorEmphasis` argument to `SourceView::new_with_asset_host`, `SourceView::new_read_only`, and the document factories that create them.

- [ ] **Step 1: Write a failing creation-and-install test**

In `source_view.rs`, construct a `SourceView` with `EditorEmphasis::Layout`, assert that the view retains `Layout`, install a real snapshot into a mounted body, and assert:

```rust
assert_eq!(body.markdown_editor().emphasis(), EditorEmphasis::Layout);
```

- [ ] **Step 2: Run the test and confirm RED**

```bash
rtk cargo test -p waml-editor source_view_copies_and_applies_its_creation_emphasis -- --nocapture
```

Expected: compilation fails because `SourceView` has no `emphasis` field and its constructor has no emphasis argument.

- [ ] **Step 3: Store and apply the copied value**

Add `emphasis: EditorEmphasis` to `SourceView` and both constructors. In `install_snapshot`, immediately after acquiring `body.markdown_editor()`, call:

```rust
editor.set_emphasis(cx, self.emphasis);
```

This placement applies retained state after remounts and relies on the editor's existing same-value no-op.

- [ ] **Step 4: Load once into `App` and pass copies through factories**

Add `markdown_emphasis: EditorEmphasis` to `App`. Set it from `crate::config::markdown_emphasis()` during startup before documents can open. Add the argument beside the existing asset-host argument on all factories that can create a `SourceView`, and pass it unchanged through navigation, history traversal, workspace preparation, OKF documents, UML documents, `GenericOkfView`, and `SourceToggleView`.

Keep test-only convenience constructors code-first with `EditorEmphasis::Code`.

- [ ] **Step 5: Verify creation paths and commit**

```bash
rtk cargo test -p waml-editor source_view::tests -- --nocapture
rtk cargo test -p waml-editor okf_documents::tests -- --nocapture
rtk cargo test -p waml-editor documents::tests -- --nocapture
rtk cargo test -p waml-editor source_toggle_view::tests -- --nocapture
git add crates/waml-editor/src
git commit -m "feat(editor): seed tab emphasis from config"
```

### Task 4: Toggle one tab in both directions

**Files:**

- Modify: `crates/waml-editor/src/source_view.rs:416-545`
- Test: `crates/waml-editor/src/source_view.rs:620-end`

**Interfaces:**

- Consumes: `HeaderViewAction` and view-owned `SourceView::emphasis`.
- Produces:

```rust
fn toggle_emphasis(&mut self, cx: &mut Cx, editor: &MarkdownEditorRef)
```

- Reuses the existing header `view_button` action.
- Does not add an app-level action or configuration write.

- [ ] **Step 1: Write failing projection and isolation tests**

Add one test that proves `Code` projects the layout destination icon and `Use layout emphasis`, while `Layout` projects `Icon::Code` and `Use code emphasis`. Add one test with two source views created from the same `Code` default. Toggle the first twice and assert after each activation that the second view and the copied session-default variable remain `Code`.

- [ ] **Step 2: Run the tests and confirm RED**

```bash
rtk cargo test -p waml-editor source_emphasis_action_projects_the_destination -- --nocapture
rtk cargo test -p waml-editor emphasis_toggle_is_two_way_and_isolated_per_tab -- --nocapture
```

Expected: the projection is absent, and compilation fails because `toggle_emphasis` does not exist.

- [ ] **Step 3: Implement the view-owned toggle and destination projection**

```rust
fn toggle_emphasis(&mut self, cx: &mut Cx, editor: &MarkdownEditorRef) {
    self.emphasis = match self.emphasis {
        EditorEmphasis::Code => EditorEmphasis::Layout,
        EditorEmphasis::Layout => EditorEmphasis::Code,
    };
    editor.set_emphasis(cx, self.emphasis);
}

fn emphasis_action(&self) -> HeaderViewAction {
    match self.emphasis {
        EditorEmphasis::Code => HeaderViewAction {
            icon: Icon::Eye,
            tooltip: "Use layout emphasis",
        },
        EditorEmphasis::Layout => HeaderViewAction {
            icon: Icon::Code,
            tooltip: "Use code emphasis",
        },
    }
}
```

In `SourceView::handle`, detect a click on the existing header view button, call `toggle_emphasis`, apply the new chrome, and return without changing the document session. Set `view_toggle: Some(self.emphasis_action())` in `SourceView::chrome`. Do not call `config::markdown_emphasis` or any configuration setter from this path.

- [ ] **Step 4: Run focused and full verification**

```bash
rtk cargo test -p waml-editor source_view::tests -- --nocapture
rtk cargo test -p waml-editor config::tests -- --nocapture
rtk cargo test -p waml-editor document_header::tests -- --nocapture
rtk cargo test -p waml-editor source_toggle_view::tests -- --nocapture
rtk cargo test -p waml-editor
rtk cargo clippy -p waml-editor --all-targets -- -D warnings
```

Expected: all tests pass and Clippy exits without warnings.

- [ ] **Step 5: Commit**

```bash
git add crates/waml-editor/src/source_view.rs
git commit -m "feat(editor): toggle markdown emphasis per tab"
```

## Self-Review

- Spec coverage is complete: absent-field default, both serialized values, creation-time copy, mount application, destination projection, two-way toggle, per-tab isolation, and unchanged session default have focused tests.
- The normal configuration-load failure path remains the only invalid-value fallback.
- The header retains one action slot. `SourceToggleView` uses it for eye/source; a direct `SourceView` uses it for emphasis.
- Type names are consistent: serialized `MarkdownEmphasis`, runtime `EditorEmphasis`, and chrome `HeaderViewAction`.
- The plan contains no deferred implementation markers.
