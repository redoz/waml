# Native Diagram Properties Form Layout Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give the native Diagram Properties page a compact, left-anchored responsive form and preserve multiline Note content through editing and WAML round trips.

**Architecture:** Keep the page shell, header, background, and vertical scrolling full-width, but place the controls inside a padded `flow: Right` body whose single form child uses Makepad's bounded `Fill{max: 620.0}` width. Normalize editor input to LF, teach the Rust and TypeScript frontmatter codecs to escape/decode line breaks in quoted strings, and let the existing `DiagramSet` operation persist the normalized value.

**Tech Stack:** Rust, Makepad live design, WAML frontmatter/operations/parser, TypeScript, Vitest, Cargo.

## Global Constraints

- Work only in `C:\dev\waml\.worktrees\diagram-properties-alignment` on branch `codex/diagram-properties-alignment`.
- Do not create another worktree, alter the main checkout, revert unrelated changes, merge, or push.
- Prefix every shell command with `rtk`.
- Keep the Note first, followed by Attributes, Relationships, and Stereotypes; omit the redundant Identity heading while preserving the independent attribute/connector cardinality semantics.
- Keep the full-height, full-width properties background and header, and keep the tool dock hidden in properties mode.
- Use 22 logical-pixel body gutters and a left-anchored form with maximum width 620 logical pixels.
- Let the form shrink to the available width without a horizontal scrollbar.
- Keep the attribute-cardinality control right-aligned with a 280 logical-pixel maximum so it can shrink on narrow layouts, and use a 72×26 logical-pixel SelectBox for Max attributes with exactly `All, 1 … 10`.
- Use a compact 11-unit medium page heading and the existing 10-unit section headings.
- Keep diagram naming in its existing external editing path: do not expose a duplicate Title field in Diagram Properties.
- Use compact 10-unit regular `fonts.text_menu` for the Note caption, Note input, property-row labels, cardinality-segment labels, and Max attributes selection.
- Keep property rows approximately 26 logical pixels high.
- Keep the native vertical scrollbar thumb visibly distinct from the panel background whenever content overflows.
- Let the multiline Note inherit Makepad's stock `TextInput` scrollbar, and use a lighter compact frame for the Max attributes flyout without changing other SelectBox flyouts.
- Reclaim the unused leading-icon gutter for `SelectLead::None` rows so the selected check never overlaps `All` or numeric labels without widening the compact flyout.
- Hide the inspector toggle and close an open inspector while Diagram Properties mode is active.
- Make Note a multiline `TextInput` with `is_multiline: true`, `Fit` height from a three-line 46 logical-pixel minimum through a 100 logical-pixel maximum, built-in overflow scrolling, and placeholder `Optional note`.
- Normalize CRLF and bare CR to LF; never flatten line breaks to spaces.
- Do not alter global font tokens.

---

### Task 1: Make quoted frontmatter strings newline-safe in Rust and TypeScript

**Files:**
- Modify: `crates/waml/src/frontmatter.rs`
- Modify: `packages/okf/src/slug.ts`
- Test: `crates/waml/src/frontmatter.rs`
- Test: `packages/okf/test/slug.test.ts`

**Interfaces:**
- Consumes: arbitrary `FmValue::Str` / JavaScript string values.
- Produces: quoted scalar text where `\n`, `\r`, `\`, and `"` are escaped without ambiguity.
- Produces: decoded strings whose CRLF and bare CR line endings are normalized to LF.

- [ ] **Step 1: Write failing Rust codec tests**

Add focused tests proving that an `FmValue::Str("First\r\nSecond\rThird\nFourth")` renders to one physical frontmatter line with escaped line breaks and reparses as `"First\nSecond\nThird\nFourth"`, while a literal backslash followed by `n` remains literal:

```rust
#[test]
fn quoted_multiline_strings_round_trip_as_normalized_lf() {
    let fm = Frontmatter {
        entries: vec![(
            "description".into(),
            FmValue::Str("First\r\nSecond\rThird\nFourth".into()),
        )],
    };
    let rendered = render_frontmatter(&fm);
    assert_eq!(
        rendered,
        r#"description: "First\nSecond\nThird\nFourth""#
    );
    let (parsed, _) = parse_frontmatter(&format!("---\n{rendered}\n---\n"));
    assert_eq!(
        parsed.get_str("description"),
        Some("First\nSecond\nThird\nFourth")
    );
}

#[test]
fn quoted_escape_decoder_distinguishes_newline_from_literal_backslash_n() {
    let fm = Frontmatter {
        entries: vec![("description".into(), FmValue::Str(r"Line\nLiteral".into()))],
    };
    let rendered = render_frontmatter(&fm);
    let (parsed, _) = parse_frontmatter(&format!("---\n{rendered}\n---\n"));
    assert_eq!(parsed, fm);
}
```

- [ ] **Step 2: Run the focused Rust tests and verify RED**

Run:

```powershell
rtk cargo test -p waml frontmatter::tests::quoted_
```

Expected: the multiline case fails because rendering writes physical line breaks and parsing splits them; the literal escape case guards the decoder design.

- [ ] **Step 3: Implement the minimal Rust quoted-string codec**

Add private helpers in `frontmatter.rs` that:

```rust
fn normalize_line_endings(value: &str) -> String
fn escape_quoted_string(value: &str) -> String
fn decode_quoted_string(value: &str) -> String
```

`escape_quoted_string` must scan normalized characters and emit `\\`, `\"`, `\n`, and `\r` escapes. `decode_quoted_string` must use a character-state scan so `\\n` decodes to a literal backslash plus `n`, while `\n` decodes to a line feed. Unknown escapes retain their backslash for compatibility.

- [ ] **Step 4: Run the focused Rust tests and verify GREEN**

Run:

```powershell
rtk cargo test -p waml frontmatter::tests::quoted_
```

Expected: both focused codec tests pass.

- [ ] **Step 5: Write failing TypeScript parity tests**

Extend `packages/okf/test/slug.test.ts`:

```typescript
it("round-trips multiline strings as normalized LF", () => {
  const text = renderFrontmatter({ description: "First\r\nSecond\rThird\nFourth" });
  expect(text).toBe('description: "First\\nSecond\\nThird\\nFourth"');
  expect(parseFrontmatter(`---\n${text}\n---\n`).data.description)
    .toBe("First\nSecond\nThird\nFourth");
});

it("distinguishes escaped newlines from a literal backslash-n", () => {
  const description = String.raw`Line\nLiteral`;
  const text = renderFrontmatter({ description });
  expect(parseFrontmatter(`---\n${text}\n---\n`).data.description)
    .toBe(description);
});
```

- [ ] **Step 6: Run the focused TypeScript tests and verify RED**

Run:

```powershell
rtk pnpm --filter @waml/okf test -- slug.test.ts
```

Expected: the multiline parity test fails against the current quote-only codec.

- [ ] **Step 7: Implement the matching TypeScript codec and verify GREEN**

Add scan-based `normalizeLineEndings`, `escapeQuotedString`, and `decodeQuotedString` helpers in `packages/okf/src/slug.ts`, use them from `scalar` and `parseValue`, then rerun:

```powershell
rtk pnpm --filter @waml/okf test -- slug.test.ts
```

Expected: all `slug.test.ts` cases pass.

- [ ] **Step 8: Commit the codec slice**

```powershell
rtk git add crates/waml/src/frontmatter.rs packages/okf/src/slug.ts packages/okf/test/slug.test.ts
rtk git commit -m "fix(waml): preserve multiline frontmatter strings"
```

---

### Task 2: Preserve multiline Diagram Notes through state, operations, parsing, and reopen

**Files:**
- Modify: `crates/waml-editor/src/diagram_properties.rs`
- Modify: `crates/waml/src/ops/mod.rs`
- Test: `crates/waml-editor/src/diagram_properties.rs`
- Test: `crates/waml/src/ops/mod.rs`

**Interfaces:**
- Consumes: `PropertyChange::Description(Option<String>)`.
- Produces: `DiagramPropertiesAction::DescriptionChanged` with CRLF/bare-CR normalized to LF and no stale diagram-title write.
- Persists: `Op::DiagramSet.description` through `apply`, `serialize_document`, and `build_model`.

- [ ] **Step 1: Replace the one-line state test with a failing LF-normalization test**

Change the existing state test to expect:

```rust
#[test]
fn description_change_preserves_lines_and_normalizes_them_to_lf() {
    let mut state = DiagramPropertiesState::new(None, resolved_display());
    let action = state.apply(PropertyChange::Description(Some(
        "First line\r\nSecond line\rThird line\nFourth line".into(),
    )));
    assert_eq!(
        action,
        DiagramPropertiesAction::DescriptionChanged(Some(
            "First line\nSecond line\nThird line\nFourth line".into()
        ))
    );
}
```

- [ ] **Step 2: Run the focused state test and verify RED**

Run:

```powershell
rtk cargo test -p waml-editor description_change_preserves_lines
```

Expected: FAIL because `normalize_description` currently replaces each line break with a space.

- [ ] **Step 3: Implement minimal LF normalization and verify GREEN**

Update `normalize_description` to preserve all non-line-ending characters, collapse CRLF to one LF, convert bare CR to LF, and continue mapping whitespace-only content to `None`. Rerun:

```powershell
rtk cargo test -p waml-editor description_change_preserves_lines
```

Expected: PASS.

- [ ] **Step 4: Replace the rejection test with a failing operation/reopen round trip**

Replace `diagram_set_rejects_multiline_description_before_serializing` with:

```rust
#[test]
fn diagram_set_round_trips_multiline_description_through_reopen() {
    let out = apply(
        &diagram_doc(),
        &[Op::DiagramSet {
            key: "dia".into(),
            title: None,
            description: Some("First line\nSecond line".into()),
            clear_description: false,
            display: None,
        }],
    )
    .unwrap();
    assert!(out[0].1.contains(r#"description: "First line\nSecond line""#));
    let reopened = crate::parse::build_model(&out);
    assert_eq!(
        reopened.diagrams[0].description.as_deref(),
        Some("First line\nSecond line")
    );
}
```

- [ ] **Step 5: Run the focused operation test and verify RED**

Run:

```powershell
rtk cargo test -p waml diagram_set_round_trips_multiline_description
```

Expected: FAIL with the current `description must be one line` operation error.

- [ ] **Step 6: Remove only the obsolete operation rejection and verify GREEN**

Delete the newline rejection guard from `op_diagram_set`; keep title, clear-description, and display behavior unchanged. Rerun:

```powershell
rtk cargo test -p waml diagram_set_round_trips_multiline_description
```

Expected: PASS, proving operation, serialization, parsing, and reopen.

- [ ] **Step 7: Commit the Note persistence slice**

```powershell
rtk git add crates/waml-editor/src/diagram_properties.rs crates/waml/src/ops/mod.rs
rtk git commit -m "fix(editor): preserve multiline diagram notes"
```

---

### Task 3: Build the compact responsive native form

**Files:**
- Modify: `crates/waml-editor/src/diagram_properties.rs`
- Test: `crates/waml-editor/src/diagram_properties.rs`

**Interfaces:**
- Produces: a full-width properties shell containing `body` and its left-anchored `form`.
- Produces: `form.walk.width == Size::Fill { max: Some(620.0), .. }`, so standard/ultrawide layouts clamp and narrow layouts consume the smaller available width.
- Produces: a multiline `description_input` and bounded `cardinality_control`.

- [ ] **Step 1: Write failing semantic widget tests**

Register the editor's `atlas`, fonts, icons, controls, and diagram-properties script modules in a test VM, instantiate `DiagramProperties`, then assert semantic widget configuration:

```rust
#[test]
fn form_uses_bounded_fill_for_responsive_left_anchored_layout() {
    let (mut vm, properties) = scripted_properties();
    let cx = vm.cx_mut();
    let walk = properties.view.widget(cx, ids!(form)).walk(cx);
    match walk.width {
        Size::Fill { max, .. } => assert_eq!(max, Some(620.0)),
        other => panic!("form width must be bounded Fill, got {other:?}"),
    }
}

#[test]
fn note_input_is_a_bounded_fit_height_multiline_editor() {
    let (mut vm, properties) = scripted_properties();
    let cx = vm.cx_mut();
    let input = properties.view.text_input(cx, ids!(description_input));
    assert!(input.borrow().expect("description input").is_multiline());
    assert!(matches!(
        input.walk(cx).height,
        Size::Fit {
            min: Some(FitBound::Abs(46.0)),
            max: Some(FitBound::Abs(100.0)),
        }
    ));
}
```

Add a third assertion that `cardinality_control.walk(cx).width` is `Size::Fill { max: Some(280.0), .. }`. The bounded `Fill` assertions are non-brittle width-clamp tests: they verify layout policy rather than rendered pixel coordinates.

- [ ] **Step 2: Run the semantic widget tests and verify RED**

Run:

```powershell
rtk cargo test -p waml-editor form_uses_bounded_fill
rtk cargo test -p waml-editor note_input_is_a_bounded_fit_height_multiline_editor
rtk cargo test -p waml-editor cardinality_control_is_bounded_and_can_shrink
```

Expected: the form child does not exist, Note is single-line at 42 logical pixels, and cardinality currently fills its row.

- [ ] **Step 3: Implement the bounded form geometry**

In the live design:

```rust
body := View {
    width: Fill
    height: Fit
    flow: Right
    align: Align{x: 0.0}
    padding: Inset{left: 22.0, right: 22.0, top: 10.0, bottom: 10.0}
    form := View {
        width: Fill{max: 620.0}
        height: Fit
        flow: Down
        spacing: 4.0
        // existing sections and controls remain in their approved order
    }
}
```

Keep scroll bars on the full-width outer `DiagramProperties`, keep the header/rule outside the bounded form, and do not add a horizontal scroll bar.

- [ ] **Step 4: Implement compact local control sizing and typography**

Apply these instance-local changes in `diagram_properties.rs`:

```rust
description_input := TextInput {
    width: Fill
    height: Fit{min: FitBound.Abs(46), max: FitBound.Abs(100)}
    is_multiline: true
    empty_text: "Optional note"
}
attribute_cardinality_row := View {
    width: Fill
    height: 30.0
    flow: Right
    cardinality_control := SegmentedControl {
        width: Fill { max: 280.0 }
    }
}
```

Set toggle/property rows to 26 logical pixels. Replace `max_attributes_input` with a bordered 72×26 `SelectBox` whose choices map `All` to `None` and `1 … 10` to their numeric limits; pass the compact flyout style so only this picker uses the thinner frame. Remove the duplicate Title label/input and redundant Identity heading, and use `fonts.text_menu` for the Note caption/input, every property-row label, all cardinality segment states, and the compact SelectBox label. Center compact control labels from their actual font metrics, use the stronger `atlas.frame_lo` color for every active Note selection state, and let Note inherit Makepad's stock multiline `TextInput` scrollbar. Use `fonts.text_label` for the compact page heading, retain the existing 10-unit section headings, style the native vertical thumb with a visible Atlas color, and make Properties-mode `BodyChrome` hide/close the inspector.

- [ ] **Step 5: Run the focused widget and state tests and verify GREEN**

Run:

```powershell
rtk cargo test -p waml-editor diagram_properties::tests
```

Expected: all Diagram Properties tests pass, including bounded Fill, compact cardinality, multiline widget configuration, newline state normalization, section order, and independent cardinality semantics.

- [ ] **Step 6: Commit the native layout slice**

```powershell
rtk git add crates/waml-editor/src/diagram_properties.rs
rtk git commit -m "feat(editor): compact diagram properties form"
```

---

### Task 4: Full verification, visual inspection, self-review, and final commit

**Files:**
- Review: every file changed since `64094a3`
- Create: screenshot files under `artifacts/diagram-properties/`

**Interfaces:**
- Consumes: the complete branch diff since `64094a3`.
- Produces: fresh test/lint evidence, standard and ultrawide screenshots, a scoped final commit history, and a clean worktree.

- [ ] **Step 1: Run formatter and focused shared-package verification**

Run:

```powershell
rtk cargo fmt --all -- --check
rtk pnpm --filter @waml/okf test
rtk pnpm --filter @waml/okf build
```

If formatting fails, run `rtk cargo fmt --all`, inspect the diff, and rerun the check.

- [ ] **Step 2: Run the required Rust verification**

Run:

```powershell
rtk cargo test -p waml-editor
rtk cargo test -p waml
rtk cargo clippy -p waml-editor --all-targets -- -D warnings
```

If a failure occurs, invoke `superpowers:systematic-debugging`, establish its root cause, add or retain the failing regression test, make one minimal correction, and rerun the failing command before continuing.

- [ ] **Step 3: Run shared core/web tests affected by frontmatter compatibility**

Run:

```powershell
rtk pnpm --filter @waml/core test
rtk pnpm --filter @waml/web test
```

Expected: all shared persistence consumers remain green.

- [ ] **Step 4: Launch at standard width and capture**

Create `artifacts/diagram-properties`, launch the native editor in properties mode using the repository's established fixture/open arguments, size its window to a standard desktop width, then run:

```powershell
rtk proxy pwsh -File scripts/capture-window.ps1 -Out artifacts/diagram-properties/native-properties-standard.png -Process waml-editor
```

Inspect the image and confirm the full-width header/background, 22-pixel body gutter, bounded left-anchored form, compact labels/rows, multiline Note field, approved section order, and absent tool dock.

- [ ] **Step 5: Resize to ultrawide and capture**

Resize the same native window to an ultrawide width, then run:

```powershell
rtk proxy pwsh -File scripts/capture-window.ps1 -Out artifacts/diagram-properties/native-properties-ultrawide.png -Process waml-editor
```

Inspect the image and confirm the form remains approximately 620 logical pixels wide at the left instead of stretching, the segmented control remains at most 280 logical pixels wide, and no tool dock appears.

- [ ] **Step 6: Self-review the complete diff**

Run:

```powershell
rtk git diff --check 64094a3
rtk git diff --stat 64094a3
rtk git diff 64094a3 -- crates/waml/src/frontmatter.rs crates/waml/src/ops/mod.rs crates/waml-editor/src/diagram_properties.rs packages/okf/src/slug.ts packages/okf/test/slug.test.ts docs/superpowers/plans/2026-07-27-native-diagram-properties-form-layout.md
```

Check every approved requirement against the diff, verify no unrelated file changed, confirm no global typography token changed, and confirm no temporary instrumentation or generated output is staged.

- [ ] **Step 7: Commit any final scoped corrections and verify clean state**

If self-review required corrections, repeat their focused RED–GREEN test cycle, rerun the affected full verification commands, then commit:

```powershell
rtk git add docs/superpowers/plans/2026-07-27-native-diagram-properties-form-layout.md crates/waml/src/frontmatter.rs crates/waml/src/ops/mod.rs crates/waml-editor/src/diagram_properties.rs packages/okf/src/slug.ts packages/okf/test/slug.test.ts
rtk git commit -m "test(editor): verify diagram properties form"
rtk git status --short --branch
```

Expected: branch `codex/diagram-properties-alignment` with a clean worktree. Do not merge or push.
