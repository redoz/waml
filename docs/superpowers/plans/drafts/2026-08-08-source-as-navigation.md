# Source as Navigation Implementation Plan

> **DRAFT — DO NOT IMPLEMENT.** Written against `DocumentKind`, before the
> decision to land `2026-08-08-surface-routed-navigation-design.md` first.
> Tasks 3 and 4 dispatch on `DocumentKind`, which that work deletes, and the
> `no_source` field introduced here is replaced there by "the `source` surface
> does not resolve for this target". Rewrite against the post-surface shapes
> before implementing. Kept in `drafts/` for the research below, verified
> against the worktree on 2026-08-08 and largely order-independent:
>
> - The `GenericOkfView` discovery (discrepancy 2) — a SECOND in-place source
>   flip the design spec never mentioned. Survives the reordering intact and
>   must be handled whenever this lands.
> - The verified touch-point table, and the confirmation that spec §5's cited
>   preview-slot tests are real (discrepancy 1).
> - The `-D warnings`/`dead_code` sequencing hazard and the deferred visual list.
>
> Two review fixes were agreed but deliberately not applied, since the rewrite
> supersedes them. Fold them in then:
>
> 1. Task 4 Step 2 hand-rolls a widget borrow that already exists as
>    `BodyWidgets::markdown_viewer_source_toggle` (`doc_view.rs:253-259`) —
>    reuse it rather than duplicating it.
> 2. Tasks 1 and 2 each push a commit to `origin/main` in which some tab kinds
>    have no source affordance at all, restored only by Task 4. Reordering does
>    not cleanly avoid this; it belongs in the plan rather than being discovered
>    mid-run.

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Spec:** `docs/superpowers/specs/2026-08-08-source-as-navigation-design.md` (approved).
**Out of scope by decree:** everything in `docs/superpowers/specs/2026-08-08-surface-routed-navigation-design.md` (surface registry routing) — do not implement any of it.

**Goal:** One view-source mechanism. The header's `FileCode`/`Eye` toggle is a shell-owned *navigation* between a document's primary tab and its `DocumentKind::Source` tab, replacing the in-place flip that `SourceToggleView` (and `GenericOkfView`'s private copy of the same idea) performs today.

**Architecture:** Delete `SourceToggleView`; strip `GenericOkfView`'s in-place flip; replace `DocumentHeaderChrome::view_toggle: Option<Icon>` with `no_source: bool` and derive the icon in the shell (`sync_document_shell`); handle the button click once in `app/actions.rs`, dispatching on the active tab's `DocumentKind` into the existing `transition_to_location` / `open_view_source` machinery. No new navigation code — tab reuse, preview-slot semantics, and view history are inherited.

**Tech stack:** Rust, makepad (`crates/waml-editor`). No `editors/vscode` source changes, but its gate still runs.

## Spec discrepancies found during research (read before implementing)

1. **§5's cited tests exist and are accurate.** `open_source_uses_the_preview_slot_and_is_a_source_tab` (`crates/waml-editor/src/doc_tabs.rs:1215`), `open_source_twice_reuses_the_same_slot_and_focuses` (`:1227`), `open_source_replaces_an_existing_preview_in_place` (`:1267`) all exist and cover preview-slot/reuse semantics at the `OpenTabs` level, and `locator_lookup_distinguishes_primary_and_source_tabs_for_one_concept` (`document_host.rs:731`) plus `promotion_app` (`app/actions.rs:1066`) cover the locator/transition level. The inherited-machinery claim holds. What is NOT covered anywhere today is the **Eye direction** (source tab → primary tab) and the **shell-owned click dispatch** — Tasks 4–5 build that coverage.
2. **The spec is silent about `GenericOkfView`**, which has its OWN in-place source flip (`generic_okf_view.rs:121-139` click handling, `:69-82` icon mutation in `sync`, backed by `ReadingView::showing_source`, `reading_view.rs:23,42-48`). Left alone it would double-handle the header click against the new shell handler. Decision taken by this plan (consistent with the spec's "one mechanism" goal): the in-place flip is removed from `GenericOkfView` too; `FileCode` on a generic OKF tab navigates to a real source tab like every other document. Task 2.
3. Spec cites `okf_documents.rs:86` for the `open_source_with_asset_host` concept-miss; the function is at `:86`, the actual `concept(...)?` miss is at `:91`. Same behavior, off-by-a-few line cite.
4. Spec's §5 says tab ids are namespaced `__doc_tab_okf__`/`__doc_tab_source__` (`okf_documents.rs:14-20`, correct) but omits that the three previously-wrapped UML views use `__doc_tab_uml__` (`uml_documents.rs:8-10`). Coexistence still holds — all three namespaces are distinct (tested at `okf_documents.rs:159`).
5. **Deleting `SourceToggleView` deletes its unit tests** (`source_toggle_view.rs:252-338`). What they covered and where the coverage goes: "opens on its own surface / identity passthrough" → dies with the wrapper (no wrapper, nothing to pass through); "toggle decorates chrome with FileCode/Eye" and "source mode hides diagram chrome" → become shell-level icon-derivation tests (Task 3) — chrome suppression is no longer needed because the source surface lives in its own tab whose `SourceView::chrome()` already declares `tool_dock/view_bar/canvas_overlays: false` (`source_view.rs:534-545`). Round-trip/escape behavior → replaced by app-level navigation tests (Task 5).

## Global constraints

- **Gate for every task, all of it, every time:**
  - `cargo test --workspace`
  - `cd editors/vscode && pnpm build && pnpm test && pnpm lint` (build FIRST — a stale `dist/` produces phantom typecheck errors).
- Clippy runs with `-D warnings`: `dead_code` is a hard error. Never leave a symbol orphaned at a commit boundary; each task below removes callers and callees together. Do NOT use `#[allow(dead_code)]`.
- `cargo fmt` before every commit.
- **No visual/GUI verification inside any task.** The automated gate cannot look at a screen. All screen-level checks are collected in "Deferred visual verification" at the foot of this plan and are explicitly NOT acceptance criteria for any task.
- Work only in `C:\dev\waml-source-nav` (branch `source-navigation`). Absolute paths below are rooted there.

## Verified touch points (current worktree, checked 2026-08-08)

| Path | What is there today |
|---|---|
| `crates/waml-editor/src/source_toggle_view.rs` | the whole wrapper (338 lines) + its 4 unit tests; DELETED by Task 1 |
| `crates/waml-editor/src/lib.rs:94` | `mod source_toggle_view;` |
| `crates/waml-editor/src/uml_documents.rs:85-136` | `open_with_asset_host(okf, uml, concept_id, assets)`; wraps all four arms in `SourceToggleView::new` (`:102-127`); `#[cfg(test)] open` at `:139-152` |
| `crates/waml-editor/src/documents.rs:130-142` | `open_locator_with_asset_host` — the only non-test caller of `uml_documents::open_with_asset_host` (`:137`) |
| `crates/waml-editor/src/generic_okf_view.rs` | in-place flip: `toggle_source` `:54-57`, icon mutation in `sync` `:69-82`, click branch in `handle` `:128-135`, `view_toggle` in `chrome()` `:151-155`, tests `:202-243` |
| `crates/waml-editor/src/reading_view.rs:23,42-48` | `showing_source: bool` + `showing_source()`/`set_showing_source()` — used ONLY by `GenericOkfView` |
| `crates/waml-editor/src/doc_view.rs:566-572` | `DocumentHeaderChrome { breadcrumb, right_dock, view_toggle }`, derives `Default`; `BodyChrome::HIDDEN` `:586-595`; `markdown_viewer_source_toggle` accessor `:253-259`; `apply_chrome` sets `set_view_toggle` `:272`; chrome tests `:618-672` |
| `crates/waml-editor/src/document_header.rs` | `DocumentHeaderState.view_toggle` `:225`, `for_test` `:243`, `replace_view_toggle` `:267`, `trailing_buttons_width` `:277-286`, `visible_height` `:288-294`, `set_view_toggle` `:458`, `view_toggle_button` `:473` |
| `crates/waml-editor/src/app/shell.rs:100-114` | `project_document_header(chrome, breadcrumb) -> (segments, right_dock, view_toggle)`; `sync_document_shell` `:848-888` applies it via `header.set_view_toggle` `:875` |
| `crates/waml-editor/src/app/actions.rs` | `EXCLUSIVE_ORDER` `:50-66` runs `DocumentHeader` (`:55`, handler `:477-503`) BEFORE `ActiveDocumentView` (`:58`); `apply_view_outcome.view_source` → `open_view_source` `:1017-1020`; `promotion_app` test `:1066` |
| `crates/waml-editor/src/app/navigation.rs` | `transition_document` `:405-425`, `open_view_source` `:432-441`, `transition_to_location` `:443` (`TransitionCause::UserNavigation` path) |
| `crates/waml-editor/src/document_host.rs` | `tab_id_for_locator` `:119-125`; `restore_location_with_asset_host` `:203-234` — locator hit activates (`:219-220`), miss opens `persistent: false` (`:222-230`) |
| `crates/waml-editor/src/doc_tabs.rs:155-157` | `DocTab::locator()` = `(concept_id, kind)`; `active_tab` `:260` |
| `crates/waml-editor/src/okf_documents.rs:86-111` | `open_source_with_asset_host`; concept miss returns `None` at `:91` |
| `crates/waml-editor/src/view_history.rs:25` | `DocumentLocator { concept_id, kind }`; `DocumentKind::{Primary, Source}` |
| `view_toggle` struct-literal sites that Task 3 must touch | `doc_view.rs:593,636,650,664` · `source_view.rs:542` · `behavior_doc_view.rs:1047` · `class_diagram_view.rs:979,990,1255,1306` · `classifier_preview_view.rs:147` · `folder_view.rs:457` · `document_host.rs:608` (test probe) · `generic_okf_view.rs:151,235` (Task 2 first) · `app/tests/navigation.rs:1737,1758,1768` · `app/shell.rs:113,867,875` · `document_header.rs` (state, listed above) |

---

### Task 1: Unwrap the three UML document views — delete `SourceToggleView`

The wrapper and everything only it needed go in one commit; `uml_documents::open_with_asset_host` returns the bare inner views. The header toggle temporarily disappears on diagram/behavior/classifier tabs (their own `chrome()` already declares `view_toggle: None`); Task 3 brings it back shell-derived. That inert intermediate state is deliberate and gate-green.

**Files:**
- Delete: `crates/waml-editor/src/source_toggle_view.rs`
- Modify: `crates/waml-editor/src/lib.rs:94` (drop `mod source_toggle_view;`)
- Modify: `crates/waml-editor/src/uml_documents.rs:85-152`
- Modify: `crates/waml-editor/src/documents.rs:137` (call-site signature)

**Interfaces:**
- Produces: `uml_documents::open_with_asset_host(okf: &waml::analysis::OkfAnalysis, uml: &waml::uml::Analysis, concept_id: &str) -> Option<OpenDocument>` — the `assets` parameter is REMOVED (the wrapper's `SourceView` was its only consumer; `ClassDiagramView`, `BehaviorDocView`, `ClassifierPreviewView` never take assets). Rename it `open` and collapse the old `#[cfg(test)] open` wrapper (`:138-152`) into it — after the param drop the two are identical, and keeping a distinct `open_with_asset_host` name for a function that takes no asset host is a lie. Update `documents.rs:137` to `open(okf, uml, &locator.concept_id)` and `documents.rs:118` (`#[cfg(test)] open`) accordingly.
- Consumes: nothing from other tasks.

- [ ] **Step 1: Unwrap `uml_documents`.** Replace the four `Box::new(SourceToggleView::new(inner, concept_id, assets.clone()))` arms (`uml_documents.rs:104-127`) with `Box::new(inner)` directly, rename the function to `open`, delete the `assets` parameter, delete the now-redundant `#[cfg(test)] open` wrapper, and delete the `use crate::source_toggle_view::SourceToggleView;` and the "Every UML document is markdown underneath…" comment (`:100-102`). Fix the two call sites in `documents.rs` (`:118`, `:137`) and any `uml_documents::open(...)` test callers (they already use the no-assets shape).
- [ ] **Step 2: Delete the wrapper.** Remove `crates/waml-editor/src/source_toggle_view.rs` and the `mod source_toggle_view;` line at `lib.rs:94`. Its 4 unit tests die with it — coverage is re-established in Tasks 3 and 5 (see "Spec discrepancies" item 5); do not port them verbatim.
- [ ] **Step 3: Sweep for stragglers.** `rg source_toggle_view crates/` must return nothing (the memory note about `folder-view-middleware` shows it referenced in old plans only — docs hits are fine, code hits are not). `rg "markdown_viewer_source_toggle" crates/` must now hit only `doc_view.rs:253` (the accessor) and `generic_okf_view.rs` (Task 2 removes those).
- [ ] **Step 4: Gate.** `cargo fmt` · `cargo test --workspace` · `cd editors/vscode && pnpm build && pnpm test && pnpm lint`. Expect green: nothing else compiles against the wrapper.
- [ ] **Step 5: Commit** — `refactor(editor): unwrap uml document views, delete SourceToggleView`.

### Task 2: Remove `GenericOkfView`'s in-place source flip

The spec never names `GenericOkfView`, but leaving its private flip alive would put two handlers on one button once the shell owns the click (Task 4). Remove the flip; keep everything else — its `source: SourceView` field stays, because `route_ui_event`, `capture_anchor`/`restore_anchor` (fragment scrolling via `document_host.rs:148-177`) and the snapshot installs all run through it and are unrelated to the toggle.

**Files:**
- Modify: `crates/waml-editor/src/generic_okf_view.rs`
- Modify: `crates/waml-editor/src/reading_view.rs:19-48`

**Interfaces:**
- Produces: `GenericOkfView::chrome()` returns `view_toggle: Some(Icon::FileCode)` as a **constant** (the field itself is replaced in Task 3); `ReadingView` loses `showing_source`/`set_showing_source` and the `showing_source` field.
- Consumes: Task 1 only in the sense that both must land before Task 4; no code dependency.

- [ ] **Step 1: Strip the flip from `GenericOkfView`.** Delete `toggle_source` (`:54-57`) and the `#[cfg(test)] showing_source` (`:49-52`). In `sync` (`:69-82`) keep only the reading branch: `body.show_markdown_viewer(cx);` — delete both `set_icon` calls (the shell owns the icon after Task 3; until then the header still renders whatever `chrome()` declares via `apply_chrome`/`sync_document_shell`). In `handle` (`:121-139`) delete the `markdown_viewer_source_toggle(...).clicked(actions)` branch (`:128-135`), keeping the `source.handle` delegation and the `outcome.source_edit = None` scrub. In `chrome()` replace the conditional (`:151-155`) with `view_toggle: Some(Icon::FileCode)`. Update the module doc comment on the `source` field (`:17-20`) — it is no longer "reached by the explicit source toggle"; it now exists for event routing, anchors, and snapshot installs only.
- [ ] **Step 2: Strip `ReadingView::showing_source`.** Delete the field (`reading_view.rs:23`), its doc comment, both accessors (`:42-48`), and the `showing_source: false` initializer (`:36`). Search the file for any other branch on it (there is none today — verify).
- [ ] **Step 3: Fix the tests in `generic_okf_view.rs:179-243`.** Delete `a_concept_opens_in_the_reading_view` and `the_source_toggle_switches_between_the_viewer_and_the_editor` (they test the deleted flip). Keep `generic_markdown_view_is_retained_and_read_only_by_construction` and `generic_document_hides_all_diagram_chrome_and_has_stable_accent` — the latter's expected chrome keeps `view_toggle: Some(Icon::FileCode)` unchanged.
- [ ] **Step 4: Gate** (full, as Task 1 Step 4). Watch specifically for `-D warnings` dead-code on `ReadingView` — if anything else still calls the accessors, the compile fails loudly; fix by deleting the caller's dead branch, never by `#[allow]`.
- [ ] **Step 5: Commit** — `refactor(editor): remove GenericOkfView's in-place source flip`.

### Task 3: `DocumentHeaderChrome` gains `no_source`; the shell derives the icon

Field swap plus derivation, no click handling yet (that is Task 4 — this split keeps the file-heavy task and the wiring-heavy task apart). After this task the button is VISIBLE and correctly iconed on every document tab (including source tabs, which get the `Eye` for the first time) but clicking it does nothing. Deliberate, gate-green intermediate state.

**Files:**
- Modify: `crates/waml-editor/src/doc_view.rs` (`:566-572` struct, `:593` HIDDEN, `:272` apply_chrome, tests `:618-672`)
- Modify: `crates/waml-editor/src/app/shell.rs` (`:100-114`, `:848-888`, new `derive_view_toggle`)
- Modify (mechanical field-literal fixes, all listed in the touch-point table): `source_view.rs:542`, `behavior_doc_view.rs:1047`, `class_diagram_view.rs:979,990,1255,1306`, `classifier_preview_view.rs:147`, `folder_view.rs:457`, `document_host.rs:608`, `generic_okf_view.rs` (chrome + test), `app/tests/navigation.rs:1732-1778`
- `document_header.rs` is NOT structurally changed: `DocumentHeaderState.view_toggle: Option<Icon>`, `set_view_toggle`, `view_toggle_button`, width/height math all stay — the header keeps rendering whatever icon the shell hands it. Only the *source* of that icon moves.

**Interfaces:**
- Produces:
  ```rust
  // doc_view.rs — replaces the current struct at :566
  /// Which pieces of the shared body chrome the active tab drives.
  #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
  pub struct DocumentHeaderChrome {
      pub breadcrumb: bool,
      pub right_dock: Option<Icon>,
      /// A purely virtual surface with no backing markdown sets this. Default
      /// `false` = toggle shown, because every real document has a source.
      pub no_source: bool,
  }
  ```
  ```rust
  // app/shell.rs — new pure helper, unit-testable without a Cx
  /// The header's source-toggle icon for the active tab, or `None` to hide the
  /// button. Two independent suppressions (spec §3): `no_source` is the view's
  /// explicit opt-out for a virtual surface that HAS a concept; a failed
  /// concept lookup covers the tab whose navigation would silently no-op
  /// (`open_source_with_asset_host` returns `None`, okf_documents.rs:91).
  pub(super) fn derive_view_toggle(
      kind: crate::view_history::DocumentKind,
      no_source: bool,
      concept_resolves: bool,
  ) -> Option<crate::icons::Icon> {
      if no_source || !concept_resolves {
          return None;
      }
      Some(match kind {
          crate::view_history::DocumentKind::Source => crate::icons::Icon::Eye,
          crate::view_history::DocumentKind::Primary => crate::icons::Icon::FileCode,
      })
  }
  ```
  `project_document_header` loses its third tuple element and the `view_toggle` passthrough: new signature `pub(super) fn project_document_header(chrome: DocumentHeaderChrome, breadcrumb: Option<Vec<BreadcrumbSegment>>) -> (Vec<BreadcrumbSegment>, Option<Icon>)`.
- Consumes: Task 2's constant `Some(Icon::FileCode)` in `GenericOkfView::chrome()` (now deleted along with the field).

- [ ] **Step 1: Write the failing shell tests first** (in `app/shell.rs`'s or `app/tests/navigation.rs`'s existing test module, matching where `project_document_header`'s tests live today — `app/tests/navigation.rs:1732-1778`):
  ```rust
  #[test]
  fn view_toggle_derivation_dispatches_on_kind_and_suppressions() {
      use crate::view_history::DocumentKind;
      assert_eq!(derive_view_toggle(DocumentKind::Primary, false, true), Some(Icon::FileCode));
      assert_eq!(derive_view_toggle(DocumentKind::Source, false, true), Some(Icon::Eye));
      // no_source wins even for a resolvable concept (spec §3: neither subsumes the other)
      assert_eq!(derive_view_toggle(DocumentKind::Primary, true, true), None);
      // unresolvable concept wins even without the opt-out
      assert_eq!(derive_view_toggle(DocumentKind::Primary, false, false), None);
      assert_eq!(derive_view_toggle(DocumentKind::Source, false, false), None);
  }
  ```
  Run: `cargo test -p waml-editor view_toggle_derivation` — expect FAIL (function does not exist).
- [ ] **Step 2: Swap the field.** In `doc_view.rs` replace `view_toggle: Option<Icon>` with `no_source: bool` exactly as in the Produces block (delete the old doc comment, keep the `Default` derive — the polarity argument in spec §3 is the point). Update `BodyChrome::HIDDEN` (`:593`) to `no_source: true` (a hidden shell has no document, so the explicit opt-out is the honest value even though the shell also never derives an icon for it — see Step 4).
- [ ] **Step 3: Chase the compiler through every literal site.** For each site in the touch-point list: `view_toggle: None` on a real document's chrome becomes `no_source: false` (diagram, classifier, source, behavior, folder, document_host probe — they all have real or don't-care concepts; the folder tab is covered by the failed concept lookup, spec §3); `view_toggle: Some(..)` (only `generic_okf_view.rs` after Task 2) is deleted — the struct's `no_source: false` default speaks for it. Test literals in `app/tests/navigation.rs:1737,1758,1768` and `doc_view.rs:636,650,664` and `generic_okf_view.rs:235`: replace the `view_toggle` line with `no_source: false` and drop any assertion that the chrome carries an icon (the icon is no longer a chrome fact).
- [ ] **Step 4: Move the derivation into the shell.** In `apply_chrome` (`doc_view.rs:261-283`) delete the `header.set_view_toggle(cx, chrome.document_header.view_toggle);` line (`:272`) — `apply_chrome` has no tab-kind or analysis access, so the toggle is exclusively `sync_document_shell`'s job from here on. In `project_document_header` (`shell.rs:100-114`) drop the third tuple element. In `sync_document_shell` (`shell.rs:848-888`), before the `project_document_header` call, compute:
  ```rust
  let view_toggle = self.documents.active_tab().and_then(|tab| {
      let concept_resolves = self
          .session
          .okf_analysis()
          .bundle
          .concept(&tab.concept_id)
          .is_some();
      derive_view_toggle(tab.kind, chrome.no_source, concept_resolves)
  });
  ```
  and keep the existing `header.set_view_toggle(cx, view_toggle);` (`:875`) fed by it. No active tab → `None` → button hidden, which also covers `BodyChrome::HIDDEN`.
- [ ] **Step 5: Update the projection tests** at `app/tests/navigation.rs:1732-1778` for the two-element tuple, and the chrome-shape tests at `doc_view.rs:618-672` for the new field (the `generic` expectation at `:655-667` loses its `Some(Icon::FileCode)` — that fact now lives in Step 1's derivation test).
- [ ] **Step 6: Run** `cargo test -p waml-editor` until green, then the full gate (as Task 1 Step 4).
- [ ] **Step 7: Commit** — `refactor(editor): shell-derived source toggle icon, DocumentHeaderChrome::no_source`.

### Task 4: The shell owns the click

Wire the button to navigation in `app/actions.rs`. Small on purpose — Task 3 carried the files, this carries the wiring.

**Files:**
- Modify: `crates/waml-editor/src/app/actions.rs:477-503` (`handle_document_header_action`)
- Test: same file's `#[cfg(test)] mod tests` (pattern: `promotion_app`, `actions.rs:1066-1097`)

**Interfaces:**
- Consumes: `derive_view_toggle` (Task 3) only conceptually — the click handler must apply the SAME suppressions, so a suppressed button that somehow still gets a click is a no-op. Uses existing `DocumentHeader::view_toggle_button` (`document_header.rs:473`), `DocTab { concept_id, kind }`, `open_view_source` (`app/navigation.rs:432`), `transition_to_location` (`:443`).
- Produces: header toggle clicks navigate; consumed flow stops the batch before `ActiveDocumentView` (`EXCLUSIVE_ORDER`, `actions.rs:50-66` — `DocumentHeader` at position 5 already precedes it; do not reorder anything).

- [ ] **Step 1: Write the failing test** (uses the headless `Cx` pattern from `promotion_app`; that helper builds a real session with one `Runbook` concept `"order"`):
  ```rust
  #[test]
  fn header_toggle_navigates_between_primary_and_source() {
      let (mut cx, mut app, _primary_id) = promotion_app();
      // promotion_app leaves the PRIMARY tab active and the source tab open+promoted.
      let primary = app.documents.active_tab().unwrap().locator();
      assert_eq!(primary.kind, crate::view_history::DocumentKind::Primary);

      // FileCode direction: primary -> existing source tab (reuse, no new tab).
      let tabs_before = app.documents.tabs().len();
      app.toggle_view_source(&mut cx);
      let active = app.documents.active_tab().unwrap();
      assert_eq!(active.kind, crate::view_history::DocumentKind::Source);
      assert_eq!(active.concept_id, "order");
      assert_eq!(app.documents.tabs().len(), tabs_before);

      // Eye direction: source -> the same concept's primary tab (reuse).
      app.toggle_view_source(&mut cx);
      let active = app.documents.active_tab().unwrap();
      assert_eq!(active.kind, crate::view_history::DocumentKind::Primary);
      assert_eq!(active.concept_id, "order");
      assert_eq!(app.documents.tabs().len(), tabs_before);
  }
  ```
  Run: `cargo test -p waml-editor header_toggle_navigates` — expect FAIL (`toggle_view_source` does not exist).
- [ ] **Step 2: Implement.** Add to the `impl App` block in `actions.rs` (next to `handle_document_header_action`) a testable seam plus the click wiring:
  ```rust
  /// Header source-toggle: a NAVIGATION between the active tab's primary and
  /// source documents (spec §2). Dispatches on the tab's kind; the concept id
  /// is the value already on the tab — nothing is resolved at click time.
  pub(super) fn toggle_view_source(&mut self, cx: &mut Cx) {
      let Some(tab) = self.documents.active_tab() else {
          return;
      };
      let (concept_id, kind) = (tab.concept_id.clone(), tab.kind);
      // Mirror the shell's suppression: a click on a button the derivation
      // would have hidden (stale frame, missing concept) must not half-navigate.
      if self
          .session
          .okf_analysis()
          .bundle
          .concept(&concept_id)
          .is_none()
      {
          return;
      }
      match kind {
          crate::view_history::DocumentKind::Source => {
              self.transition_to_location(
                  cx,
                  ViewLocation {
                      document: crate::navigation::DocumentLocator::primary(&concept_id),
                      anchor: ViewAnchor::None,
                  },
                  TransitionCause::UserNavigation,
              );
          }
          crate::view_history::DocumentKind::Primary => {
              self.open_view_source(cx, &concept_id);
          }
      }
  }
  ```
  In `handle_document_header_action` (`:477`), before the existing `action` match, check the button:
  ```rust
  let toggle_clicked = self
      .ui
      .widget(cx, ids!(document_header))
      .borrow::<crate::document_header::DocumentHeader>()
      .map(|header| header.view_toggle_button(cx))
      .is_some_and(|button| button.as_icon_button().clicked(actions));
  if toggle_clicked {
      self.toggle_view_source(cx);
      return ActionFlow::Consumed;
  }
  ```
  (`IconButtonWidgetRefExt` is already imported where needed — check `use` lists; `as_icon_button().clicked` is the exact pattern the deleted views used.)
- [ ] **Step 3: Note the `no_source` gap deliberately left open.** `toggle_view_source` re-checks the concept lookup but NOT `no_source` — no shipped view sets it yet (it exists for future virtual surfaces), and the button is never rendered when it is set, so there is no click to receive. If a reviewer wants belt-and-braces, `self.documents.active_chrome().document_header.no_source` is the check; do not add it speculatively.
- [ ] **Step 4: Run the new test, then the full gate** (as Task 1 Step 4).
- [ ] **Step 5: Commit** — `feat(editor): header source toggle navigates between primary and source tabs`.

### Task 5: App-level coverage for the inherited semantics

The spec's Testing section, minus what already exists and minus anything visual. These are the tests that replace `SourceToggleView`'s dead coverage at the level the behavior now lives.

**Files:**
- Test: `crates/waml-editor/src/app/tests/navigation.rs` (pattern: `mounted_source_app` `:212-243`, `navigation_app`, the history tests around `:761-965`)
- Test: `crates/waml-editor/src/app/actions.rs` tests (extend Task 4's module)

**Interfaces:**
- Consumes: `App::toggle_view_source` (Task 4), `derive_view_toggle` (Task 3).

- [ ] **Step 1: Preview-slot on a cold toggle.** In `actions.rs` tests: build an app with concept `"order"` (reuse `promotion_app`'s session-setup lines, but do NOT open a source tab), open the primary preview via `app.transition_document(&mut cx, "order", false)`, then `app.toggle_view_source(&mut cx)`; assert the active tab is `DocumentKind::Source`, `preview == true` (`app.documents.active_tab().unwrap().preview`), and the tab count did not grow (the source preview replaced the primary preview in the slot — `open_source_replaces_an_existing_preview_in_place` semantics, now end-to-end).
- [ ] **Step 2: Back returns to the departing document.** In `app/tests/navigation.rs`, following the existing history-test pattern (`:761` onward: `transition_to_location` + `navigate_back`-style calls — match the file's actual helper names when writing): open primary `"notes/order"` (persistent), `toggle_view_source`, assert active kind `Source`; drive history back; assert the active tab is the primary `"notes/order"` again. This pins spec §5's "back after a toggle returns to the departing document".
- [ ] **Step 3: Suppression end-to-end.** In `app/tests/navigation.rs` near the `project_document_header` tests (`:1732`): with a folder/directory tab active (concept lookup misses — see `sync_document_shell`'s directory fallback comment at `shell.rs:857-862`), assert `derive_view_toggle(tab.kind, chrome.no_source, false)` yields `None` via the shell path if a mounted-header helper exists (`assert_mounted_header`, `:1780`), otherwise at the `derive_view_toggle` level with inputs captured from the real tab — do not build new widget-mounting machinery for this.
- [ ] **Step 4: Toggle on each previously-wrapped kind opens a source tab.** Parameterized over the three fixtures the repo already uses (`uml.Class` classifier, `uml.Activity` behavior, a diagram key — see `uml_documents.rs` tests `:159-191` for the frontmatter shapes): open the primary tab, `toggle_view_source`, assert active `(concept_id, DocumentKind::Source)` and that toggling again returns to `(concept_id, DocumentKind::Primary)`. Skip any assertion about pixels, surfaces, or canvas state — that is the deferred visual pass.
- [ ] **Step 5: Full gate** (as Task 1 Step 4).
- [ ] **Step 6: Commit** — `test(editor): app-level coverage for source-as-navigation semantics`.

---

## Deferred visual verification (post-implementation, human-run — NOT a task, NOT gate criteria)

The automated workflow cannot look at a screen. Each item below ships gate-green with the check outstanding; the plan is not signed off until a human walks this list in a real window (the prior toggle work was verified by synthetic clicks at the header's top-right; the same method applies — see `interactive-verify-by-synthetic-clicks` notes).

- [ ] `FileCode` on a class-diagram tab, a behavior tab, and a classifier-preview tab opens a source TAB (tab strip changes) rather than flipping the body in place; diagram chrome (tool dock, view bar, conflict badge) never appears over the source editor.
- [ ] `Eye` on a source tab returns to that concept's primary tab; the icon on each side shows the surface the toggle LEADS to.
- [ ] `FileCode` on a generic OKF (Runbook-style) tab now opens a source tab — this is a deliberate behavior change from the old in-place flip (Task 2 decision).
- [ ] The button is absent on a folder/index tab (concept lookup miss) and absent when no document is active.
- [ ] Escape in a source tab no longer "exits source mode" — the tab stays open; Escape does whatever the source editor does.
- [ ] Header layout: the trailing-button row (`trailing_buttons_width`, `document_header.rs:277`) reserves space correctly with the toggle now present on source tabs too (it previously never was).

## Self-review notes

- Spec §1 (delete wrapper) → Task 1. §2 (shell owns toggle) → Task 4. §3 (`no_source`) → Task 3. §4 (source tabs gain Eye) → Task 3 (derivation) + Task 4 (Eye dispatch). §5 (inherited semantics) → verified existing tests + Task 5. Decisions-taken section: concept-from-tab-locator (Task 4 code), preview disposition (inherited, Task 5 Step 1), Escape (deletion, visual list). Testing section: every non-visual bullet has a task; visual bullets are in the deferred list.
- Orphan check at each boundary: T1 deletes wrapper + its only constructor calls together; T2 deletes `ReadingView` accessors and their only caller together; T3's field swap is compile-enforced across all literal sites in one commit; T4 adds `toggle_view_source` and its caller + test in one commit.
- `GenericOkfView` decision (discrepancy 2) is the one thing here the spec did not settle; it is flagged, minimal (affordance removed, plumbing kept), and reversible.
