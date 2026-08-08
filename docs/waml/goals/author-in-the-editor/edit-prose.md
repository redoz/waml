# Edit Prose

**Goal:** An author writes the Markdown body of a document in the editor.

**Why:** Most bundle content is text. The editor must let the author change that
text without a separate application.

**Done when:** The author can insert and select text, use the clipboard, keep
more than one caret, use an input method, and continue safely when layout data
is stale.

**Status:** done
**MVP:** yes

## Shipped behavior

#### NATIVE-023 — typed text updates an editable Markdown document

**Applies to:** shared

**Given** an editable Markdown session has a current source revision
**When** the author types text
**Then** the editor proposes the exact updated source for the next revision

**Evidence:** `crates/waml-editor/tests/markdown_integration.rs::editable_mount_emits_an_exact_revisioned_source_proposal`

#### NATIVE-024 — clipboard edits respect the session access mode

**Applies to:** shared

**Given** the same source text is selected in writable and read-only sessions
**When** the author copies the selection and tries to cut it
**Then** both sessions copy the exact source text
**And** only the writable session removes the selection

**Evidence:** `crates/waml-markdown-editor/tests/widget_parity.rs::retained_copy_cut_paste_use_source_text` `crates/waml-markdown-editor/tests/widget_parity.rs::read_only_mode_allows_selection_and_copy_but_not_mutation`

#### NATIVE-025 — a modifier action adds a selection

**Applies to:** shared

**Given** a Markdown document has an existing selection
**When** the author uses the platform selection modifier at another text position
**Then** the existing selection remains and the new selection is added

**Evidence:** `crates/waml-markdown-editor/tests/widget_parity.rs::retained_platform_modifier_adds_selection` `crates/waml-markdown-editor/tests/widget_parity.rs::mounted_widget_primary_modifier_adds_a_selection`

#### NATIVE-026 — an IME composition has visible preedit and reversible completion

**Applies to:** shared

**Given** an editable Markdown session has a source selection
**When** the author starts and updates an IME composition, then commits or cancels it
**Then** preedit text is visible without publishing a source revision
**And** commit publishes the composed text while cancel restores the committed source and selection

**Evidence:** `crates/waml-markdown-editor/tests/unicode_ime.rs::ime_preedit_is_visible_state_but_not_a_published_revision` `crates/waml-markdown-editor/tests/unicode_ime.rs::ime_cancel_restores_the_last_committed_snapshot_and_selection` `crates/waml-markdown-editor/tests/unicode_ime.rs::ime_replaces_nonempty_selection_and_cancel_models_focus_loss`

#### NATIVE-046 — pointer clicks select a source word or line

**Applies to:** shared

**Given** a Markdown document contains source text
**When** the author double-clicks a word or triple-clicks a source line
**Then** the editor selects the complete word or source line at that position

**Evidence:** `crates/waml-markdown-editor/tests/widget_parity.rs::retained_double_click_selects_word` `crates/waml-markdown-editor/tests/widget_parity.rs::retained_triple_click_selects_source_line`

#### NATIVE-047 — stale layout input does not change the document

**Applies to:** shared

**Given** the Markdown layout revision differs from the current session revision
**When** the author sends input through that stale layout
**Then** the editor rejects the input and keeps the current document unchanged

**Evidence:** `crates/waml-markdown-editor/src/input.rs:416`

## Verification gaps

- NATIVE-047 — target: native; No native integration test asserts the stale-layout rejection result.

## Notes

- The scenarios use prose for semantic text positions, multiple carets, and IME
  composition. [FG-005](../../waml-feature-gaps.md#fg-005--semantic-text-positions-and-ime-composition)
  records this WAML expression gap.
- The scenarios describe typed gestures and visible results in prose.
  [FG-002](../../waml-feature-gaps.md#fg-002--typed-gestures-and-consumed-input)
  records this WAML expression gap.
- [Round-Trip Losslessly](../trust-the-content/round-trip-losslessly.md)
  owns unchanged-source byte accuracy.
