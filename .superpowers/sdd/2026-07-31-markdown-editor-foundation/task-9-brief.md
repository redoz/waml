### Task 9: Add platform-neutral input, read-only behavior, and caret scrolling

**Files:**
- Create: `crates/waml-markdown-editor/src/input.rs`
- Modify: `crates/waml-markdown-editor/src/session.rs`
- Create: `crates/waml-markdown-editor/tests/widget_parity.rs`

**Interfaces:**
- Consumes: session command lowering and `LayoutSnapshot` queries.
- Produces: `EditorInput`; `SelectionModifier`; `PointerGesture`; `EditorResponse`; `ScrollState`; `ScrollAnchor`; `ScrollAdjustment`; `MarkdownEditorController::handle`.

- [ ] **Step 1: Write failing retained-behavior characterization tests**

Create `tests/widget_parity.rs`:

```rust
#[test]
fn click_drag_double_and_triple_click_match_retained_editor_behavior() {
    let mut fixture = Fixture::new("alpha beta\nsecond\n");
    fixture.click_at_offset(2, 1, SelectionModifier::Replace);
    assert!(fixture.primary().is_empty());
    fixture.drag_to_offset(5);
    assert_eq!(fixture.selected_text(), "pha");
    fixture.click_at_offset(8, 2, SelectionModifier::Replace);
    assert_eq!(fixture.selected_text(), "beta");
    fixture.click_at_offset(13, 3, SelectionModifier::Replace);
    assert_eq!(fixture.selected_text(), "second\n");
}

#[test]
fn platform_modifier_adds_selection_and_shift_extends_primary() {
    let mut fixture = Fixture::new("one two");
    fixture.click_at_offset(1, 1, SelectionModifier::Replace);
    fixture.click_at_offset(5, 1, SelectionModifier::Add);
    assert_eq!(fixture.session().selections().as_slice().len(), 2);
    fixture.click_at_offset(7, 1, SelectionModifier::Extend);
    assert_eq!(fixture.selected_text(), "two");
}

#[test]
fn read_only_mode_allows_selection_and_copy_but_not_mutation() {
    let mut fixture = Fixture::new("raw *markdown*");
    fixture.session_mut().set_read_only(true);
    fixture.select_all();
    assert_eq!(fixture.copy(), "raw *markdown*");
    let response = fixture.type_text("x");
    assert!(response.proposals.is_empty());
    assert_eq!(fixture.text(), "raw *markdown*");
}

#[test]
fn caret_visibility_and_resize_use_geometry_not_line_numbers() {
    let mut fixture = Fixture::with_variable_layout();
    fixture.set_viewport(100.0, 40.0);
    fixture.place_caret_at_end();
    let first = fixture.ensure_caret_visible();
    assert!(first.scroll_y > 0.0);
    fixture.resize_width(50.0);
    let second = fixture.ensure_caret_visible();
    assert!(second.scroll_y >= first.scroll_y);
}
```

- [ ] **Step 2: Run and verify the controller is absent**

Run: `rtk cargo test -p waml-markdown-editor --test widget_parity`

Expected: FAIL with unresolved input/controller types.

- [ ] **Step 3: Implement typed input and response**

Define:

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SelectionModifier {
    Replace,
    Extend,
    Add,
}

#[derive(Clone, Debug, PartialEq)]
pub enum EditorInput {
    Text(Arc<str>),
    Paste(Arc<str>),
    Copy,
    Cut,
    Key(EditorKey),
    PointerDown(PointerGesture),
    PointerMove { point: DVec2 },
    PointerUp,
    ImeStart,
    ImeUpdate { preedit: String, selection: Range<u32> },
    ImeCommit,
    ImeCancel,
}

#[derive(Default)]
pub struct EditorResponse {
    pub proposals: Vec<ProposedMarkdownEdit>,
    pub clipboard: Option<String>,
    pub request_redraw: bool,
    pub request_ime_at: Option<DVec2>,
}
```

Map normal click to caret, drag/Shift to extension, double-click to Unicode word, triple-click to logical source line, and the platform add-selection modifier to a new selection. Copy always concatenates literal selected Markdown source in selection order. Cut, paste, typing, Enter, Tab, Shift-Tab, Delete, Backspace, undo, redo, and IME commit all call the same session transaction APIs. In read-only mode, preserve focus, navigation, selection, scrolling, and copy while returning no edit proposal.

- [ ] **Step 4: Implement scroll state and anchoring**

Define:

```rust
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ScrollState {
    pub x: f64,
    pub y: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScrollAnchor {
    pub position: TextPosition,
    pub viewport_y: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScrollAdjustment {
    pub scroll_y: f64,
}
```

`ensure_primary_caret_visible` uses `LayoutSnapshot::source_to_point` plus one caret-height pad above and two below. `capture_scroll_anchor` records the primary caret's viewport y. `restore_scroll_anchor` computes the new scroll y from the new layout geometry after edits, font changes, embedded-block measurement, or viewport resize. Clamp only scroll coordinates, never source offsets.

Add `scroll: ScrollState` to `MarkdownDocumentSession` and initialize it to `ScrollState::default()`.

- [ ] **Step 5: Run parity tests**

Run: `rtk cargo test -p waml-markdown-editor --test widget_parity`

Expected: PASS, 4 tests.

- [ ] **Step 6: Commit controller and scroll behavior**

```bash
rtk git add crates/waml-markdown-editor/src/input.rs crates/waml-markdown-editor/src/session.rs crates/waml-markdown-editor/tests/widget_parity.rs
rtk git commit -m "feat: add markdown editor input controller"
```
