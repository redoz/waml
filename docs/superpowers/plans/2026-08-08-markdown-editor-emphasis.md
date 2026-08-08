# Markdown Editor Emphasis Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add code-first and layout-first emphasis profiles to the markdown editor, with code emphasis as the default.

**Architecture:** `EditorEmphasis` is public intent. `PresentationStyles` resolves that intent into complete typography and spacing values, and the widget rebuilds its installed layout document when emphasis changes. Parsing, presentation items, document revisions, selections, and decorations remain shared.

**Tech Stack:** Rust, Makepad live widgets, `waml-markdown-editor` presentation and layout pipeline, Cargo tests, native editor harness.

## Global Constraints

- Work only in `C:\dev\waml\.worktrees\markdown-editor-emphasis`.
- Use ASD-STE100 Simplified Technical English in code comments and documentation.
- `Code` is the default. Backward compatibility with the current visual default is not required.
- `Code` uses the project mono family and zero base vertical row spacing while it keeps the existing horizontal inset.
- Both profiles preserve semantic size, weight, italic, underline, strikethrough, links, diagnostics, inline decorations, and variable row height.
- Launch every visual check with `run.ps1 -Title markdown-editor-emphasis`.

---

### Task 1: Resolve emphasis into presentation styles

**Files:**
- Modify: `crates/waml-markdown-editor/src/presentation/style.rs`
- Modify: `crates/waml-markdown-editor/src/presentation/mod.rs`
- Modify: `crates/waml-markdown-editor/src/lib.rs`
- Test: `crates/waml-markdown-editor/tests/presentation_style.rs`
- Test: `crates/waml-markdown-editor/tests/presentation_layout.rs`

**Interfaces:**
- Produces: `pub enum EditorEmphasis { Code, Layout }` with `Default` returning `Code`.
- Produces: `PresentationStyles::for_emphasis(emphasis: EditorEmphasis) -> Self`.
- Produces: `PresentationStyles::emphasis(&self) -> EditorEmphasis`.
- Preserves: `PresentationStyles::balanced() -> Self` as the explicit `Layout` constructor until all internal callers migrate.

- [ ] **Step 1: Write failing profile tests**

Add tests that express the resolved behavior, not private fields:

```rust
#[test]
fn code_emphasis_is_default_and_uses_mono_metrics() {
    let styles = PresentationStyles::default();
    assert_eq!(styles.emphasis(), EditorEmphasis::Code);
    assert_eq!(styles.metrics(TextRole::Body).font, FONT_MONO);
    assert_eq!(styles.metrics(TextRole::Strong).font, FONT_MONO);
    assert_eq!(styles.metrics(TextRole::Emphasis).font, FONT_MONO);
}

#[test]
fn layout_emphasis_keeps_balanced_body_metrics() {
    let styles = PresentationStyles::for_emphasis(EditorEmphasis::Layout);
    assert_eq!(styles.metrics(TextRole::Body).font, FONT_SANS);
    assert_eq!(styles.spacing().paragraph_after, 6.0);
}
```

In `presentation_layout.rs`, build the same one-paragraph plan with both profiles. Assert that `Code` has `space_after == 0.0`, `Layout` keeps `6.0`, and both retain the same left and right document inset. Add a heading and a strong-emphasis run; assert that the heading metric remains larger and the strong run remains semibold under `Code`.

- [ ] **Step 2: Run the focused tests and verify RED**

Run:

```powershell
rtk cargo test -p waml-markdown-editor --test presentation_style --test presentation_layout
```

Expected: compilation fails because `EditorEmphasis`, `for_emphasis`, and `emphasis` do not exist.

- [ ] **Step 3: Implement the emphasis resolver**

Replace the unit `PresentationStyles` with a value that stores `EditorEmphasis`. Derive `Clone`, `Copy`, `Debug`, `Eq`, and `PartialEq` where valid. Keep all resolution in this type:

```rust
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum EditorEmphasis {
    #[default]
    Code,
    Layout,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PresentationStyles {
    emphasis: EditorEmphasis,
}

impl Default for PresentationStyles {
    fn default() -> Self {
        Self::for_emphasis(EditorEmphasis::default())
    }
}
```

Make `balanced()` return `Layout`. In `metrics`, keep the current sizes, line spacing, weights, and italic flags. Under `Code`, replace only the resolved font family with `FONT_MONO`. In `spacing`, set ordinary paragraph spacing and heading margins to zero for `Code`; keep construct-specific insets such as quote, code-block, and table-cell geometry because they are rendered decorations, not base row padding. Keep document left and right insets identical in both profiles.

- [ ] **Step 4: Run the focused tests and verify GREEN**

Run the command from Step 2. Expected: both test binaries pass with no warnings.

- [ ] **Step 5: Commit the profile resolver**

```powershell
rtk git add crates/waml-markdown-editor/src/presentation/style.rs crates/waml-markdown-editor/src/presentation/mod.rs crates/waml-markdown-editor/src/lib.rs crates/waml-markdown-editor/tests/presentation_style.rs crates/waml-markdown-editor/tests/presentation_layout.rs
rtk git commit -m "feat(markdown): add emphasis profiles"
```

### Task 2: Make emphasis a widget option and rebuild dependent layout

**Files:**
- Modify: `crates/waml-markdown-editor/src/widget.rs`
- Test: `crates/waml-markdown-editor/tests/widget_parity.rs`

**Interfaces:**
- Consumes: `EditorEmphasis` and `PresentationStyles::for_emphasis` from Task 1.
- Produces: `MarkdownEditorRef::set_emphasis(&self, cx: &mut Cx, emphasis: EditorEmphasis)`.
- Produces: `MarkdownEditorRef::emphasis(&self) -> EditorEmphasis`.

- [ ] **Step 1: Write failing widget tests**

Add a test that creates a mounted editor, installs a layout-emphasis presentation, calls `set_emphasis(..., EditorEmphasis::Code)`, and asserts:

```rust
assert_eq!(editor.emphasis(), EditorEmphasis::Code);
assert_eq!(installed.styles.emphasis(), EditorEmphasis::Code);
assert_eq!(installed.layout_document.text_runs[body_run].metrics.font, FONT_MONO);
assert_eq!(installed.layout_document.blocks[paragraph].spec.space_after, 0.0);
assert_eq!(session.snapshot().revision(), revision_before);
assert_eq!(session.selections(), &selections_before);
```

Add a second assertion path showing that calling `set_emphasis` with the current value leaves the installed `Arc` unchanged.

- [ ] **Step 2: Run the widget test and verify RED**

Run:

```powershell
rtk cargo test -p waml-markdown-editor --test widget_parity emphasis
```

Expected: compilation fails because the widget emphasis accessors do not exist.

- [ ] **Step 3: Implement widget emphasis and invalidation**

Add `emphasis: EditorEmphasis` to `MarkdownEditor` with a Rust default of `Code`. In `set_emphasis`, return early when the value is unchanged. Otherwise:

1. Build `Arc<PresentationStyles>` with `PresentationStyles::for_emphasis(emphasis)`.
2. Rebuild `LayoutDocument` with `build_layout_document`, the installed plan, and `EmbeddedMeasurements { revision: Some(installed.revision), blocks: installed.layout_document.embedded_blocks.clone() }`.
3. Build a replacement `InstalledPresentation` that reuses the plan, diagnostics, and assets.
4. Replace `pipeline.installed` only after the rebuilt presentation validates.
5. Set `target_layout = None`, `pending_cause = Some(LayoutChangeCause::ViewportResize)`, and `pending_invalidation = Some(LayoutInvalidation::ViewportWidth)`.
6. Clear typography-dependent text layout cache entries if their key does not already contain all resolved metrics.
7. Redraw without changing the session, selection, scroll, or document revision.

Return the current value from `emphasis()`. If rebuilding unexpectedly fails, keep the previous installed presentation and log the error through the widget's existing throttled error pattern; do not leave mixed profile state.

- [ ] **Step 4: Run the widget and full crate tests**

Run:

```powershell
rtk cargo test -p waml-markdown-editor --test widget_parity emphasis
rtk cargo test -p waml-markdown-editor
```

Expected: the emphasis tests pass, then all crate tests pass with no warnings.

- [ ] **Step 5: Commit the widget option**

```powershell
rtk git add crates/waml-markdown-editor/src/widget.rs crates/waml-markdown-editor/tests/widget_parity.rs
rtk git commit -m "feat(editor): apply emphasis at runtime"
```

### Task 3: Verify real rendering in the editor harness

**Files:**
- Create as scratch input: `C:\tmp\markdown-editor-emphasis\sample.md`
- Verify: `scripts/capture-window.ps1`

**Interfaces:**
- Consumes: the default `Code` widget emphasis and `set_emphasis` from Task 2.
- Produces: native-pixel evidence that code emphasis remains richly decorated and variable-height.

- [ ] **Step 1: Prepare one representative markdown fixture**

Create the scratch fixture with headings, paragraphs, strong, emphasis,
strong-emphasis, strikethrough, a link whose label supplies the underline,
inline code, an invalid link that supplies a diagnostic, a quote, and a table.
Use this exact content for both emphasis values:

```markdown
# Working with projections

A projection keeps **strong facts**, *emphasis*, and ***both together***.

Keep ~~obsolete routes~~ visible beside [source ranges](waml:source).

Use `view source` and inspect [this invalid destination](waml:).

> Decoration height still belongs to its source row.

| Mode | Rhythm |
| --- | --- |
| Code | Compact |
| Layout | Page-like |
```

- [ ] **Step 2: Launch and capture Code emphasis**

Run the editor with the required unique title:

```powershell
rtk pwsh -File run.ps1 -Title markdown-editor-emphasis
rtk pwsh -File scripts/capture-window.ps1 -Out markdown-editor-emphasis-code.png -Process waml-editor
```

Confirm the capture shows mono text, no ordinary inter-row padding, semantic
heading height, inline weight and italic differences, link underline,
strikethrough, diagnostics, and decoration-driven height.

- [ ] **Step 3: Compare Layout emphasis**

Set the harness instance to `EditorEmphasis::Layout`, relaunch with `-Title markdown-editor-emphasis`, and capture `markdown-editor-emphasis-layout.png`. Confirm page-like spacing returns and the content and inline semantics remain the same.

- [ ] **Step 4: Run workspace verification**

Run:

```powershell
rtk cargo fmt --all -- --check
rtk cargo test -p waml-markdown-editor
rtk cargo test -p waml-editor
rtk git diff --check
```

Expected: formatting passes, both crate test suites pass, and `git diff --check` reports no errors.

- [ ] **Step 5: Remove scratch visual evidence**

Close both labeled editor windows. Remove only
`C:\tmp\markdown-editor-emphasis` after resolving and checking that exact path.
Do not commit screenshot files.
