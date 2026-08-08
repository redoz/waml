# Surface-Routed Navigation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Spec:** `docs/superpowers/specs/2026-08-08-surface-routed-navigation-design.md` (approved, already corrected once against the spike at `spike/surface-seam:docs/superpowers/specs/notes/2026-08-08-surface-seam-spike.md`).
**Out of scope by decree:** everything in `docs/superpowers/specs/2026-08-08-source-as-navigation-design.md` (shell-owned toggle, `SourceToggleView` deletion, `no_source`, `GenericOkfView`'s flip). Do not touch `source_toggle_view.rs`, `generic_okf_view.rs`'s toggle, `reading_view.rs`, or `DocumentHeaderChrome`. That plan lands after this one and dispatches on the surfaces this plan creates.

**Goal:** One navigation vocabulary. `DocumentLocator` becomes `{ target: RowTarget, surface: SurfaceId }`, `DocumentKind` stops existing, the dormant surface registry becomes THE open path, and the forcing case works end to end: a folder tab's `"source"` surface is expressible, opens, and round-trips through Back.

**Architecture:** Five moves, strictly ordered so each lands green: (1) re-shape the dead `SurfaceFactory` seam onto `RowTarget` keys and `OpenDocument` returns; (2) fix the folder-tab locator round-trip on TODAY's shapes so the behavior change ships separately from the type change; (3) make the `"source"` surface target-resolving with the render-predicate gate; (4) widen the locator (a pure recompile — nothing locator-valued is persisted, verified by the spike); (5) route the open path through the registry and delete the `.or_else` provider chain. Tab-id unification and the forcing-case coverage follow.

**Tech stack:** Rust, makepad (`crates/waml-editor`), `crates/waml` (view/surface module). No `editors/vscode` source changes, but its gate still runs.

## Global constraints

- **Gate for every task, all of it, every time:**
  - `cargo fmt` (before every commit)
  - `cargo test --workspace`
  - `cd editors/vscode && npm run build && npm run test && npm run lint` (build FIRST — a stale `dist/` produces phantom typecheck errors).
- Clippy runs with `-D warnings`: `dead_code` is a hard error. The surface seam is currently blanketed in `#[allow(dead_code)]`; Task 5 strips those as the seam goes live and no task may add a new one. Items wired into an `#[allow(dead_code)]` root count as live to the lint, so Tasks 1–3 may extend the dormant seam without new allows.
- **No visual/GUI verification inside any task.** All screen-level checks live in "Deferred visual verification" at the foot of this plan and are NOT acceptance criteria for any task.
- Work only in `C:\dev\waml-source-nav` (branch `source-navigation`). Absolute paths below are rooted there.
- Each task is independently committable; where a commit changes user-visible behavior it is called out in the task header — this history pushes to `origin/main` per unit.

## Research findings — where the spec and the code disagree (read before implementing)

All line numbers verified against the worktree on 2026-08-08.

1. **`SurfaceFactory` returns `Box<dyn DocView>`, but the live open path consumes `OpenDocument`** (`tab_id`, `concept_id`, `kind`, `title`, `presentation`, `view` — `document.rs:70-77`). Neither the spec nor the spike resolves this. A factory that hands back a bare view cannot produce a tab. **Decision taken by this plan:** the factory signature becomes `Fn(&OpenCtx<'_>, &RowTarget) -> Option<OpenDocument>` and each factory delegates to the existing provider function (`okf_documents::open_with_asset_host` etc.), which already builds the full `OpenDocument`. This also fixes a second, worse divergence: the current dead factories build **bare** views (`open_canvas` → `ClassDiagramView` only, `extension_editor.rs:195-198`), while the live path wraps every UML view in `SourceToggleView` and dispatches four ways by category (`uml_documents.rs:103-127`). Making the dead factories live as-written would silently drop the wrapper and three of the four view kinds.
2. **The `__legacy_edit__` sentinel is `#[cfg(test)]`-only.** `editor_session.rs:515` sits inside `apply_with_preparer` (`:500-530`), itself `#[cfg(test)]`, reached only from the `#[cfg(test)] apply` helper (`:495-498`). No production code constructs it. Resolution (Task 4): its `ViewLocation` becomes `{ target: RowTarget::Virtual, surface: SurfaceId("markdown") }` — the honest value for "an edit with no document behind it", and exactly the shape spec §1 grants Virtual ("gets a locator too, though nothing can open one"). No removal needed; no production behavior involved.
3. **`RowTarget` does not derive `Hash`** (`crates/waml/src/view/row.rs:160-168`: `Debug, Clone, PartialEq, Eq` only), but `DocumentLocator` derives `Hash` today (`view_history.rs:24`). Task 4 adds `Hash` to the `RowTarget` derive.
4. **`SurfaceId` is a newtype over a `pub String`** (`surface.rs:11`), so the spec's parenthetical "an unresolved id cannot be constructed by accident" is not enforced by the type and this plan does not make it so — sealing the field would ripple through `Row::new` callers and every middleware for no behavioral gain. The plan adds named constructors (`SurfaceId::markdown()` etc., Task 4) so editor code never spells a raw string, and leaves enforcement to `resolve_surface`.
5. **§4's "canvas" resolution needs a fallback arm.** The editor's default-surface resolution must be "canvas iff `uml.claims.contains(id)`" (matching today's `.or_else` outcome by construction), NOT `waml::view::surface::default_surface`, which classifies by `ElementType::parse` of the frontmatter type (`surface.rs:26-49`) — the two sets can disagree (an invalid-but-claimed `uml.Class` is claimed; the claim set is the arbiter of which provider owns the tab today, see `documents.rs` test `invalid_claimed_uml_stays_owned...`). Additionally a **stored** `"canvas"` locator can go stale when a concept's type is edited from `uml.Class` to something generic; today's `Primary` arm falls through to the generic provider, so the `"canvas"` open keeps `.or_else(okf_documents::open_with_asset_host)` as a degrade path (Task 5). This is the one place the old chain survives, scoped and commented.
6. **Four tab-id namespaces, not two:** `__doc_tab_okf__`/`__doc_tab_source__` (`okf_documents.rs:14-20`), `__doc_tab_uml__` (`uml_documents.rs:8-10`), `__doc_tab_folder__` (`folder_view.rs:215-217`). Spec §3 names only two; Task 7 unifies all four. Nothing tab-id-valued is persisted (spike Q4), so `LiveId` values may change.
7. **Confirmed as the spec states:** `NavigationTarget::Document` has no surface field (`navigation.rs:8-17`); `open_row_with_asset_host`'s doc comment names that as its blocker (`documents.rs:49-57`); the folder tab stamps `{concept_id: directory, kind: Primary}` (`folder_documents.rs:52-53`) and never resolves (`documents.rs:136-141` has no folder arm); `Bundle::index` synthesizes and is the wrong gate (spike Q3); `hide` is presentational (`crates/waml/src/view/hide.rs:6-11`); nothing locator-valued carries `Serialize`/`Deserialize`.
8. **The open site lacks `limits`/`mode`.** `DocumentHost::restore_location_with_asset_host` (`document_host.rs:203-234`) has `session` and `assets` but not `ChainLimits`/`ViewMode`, which are `App` fields (used at `app/navigation.rs:301-302`, `:716-717`). Task 2 plumbs them as parameters (spike Q1's "two lines of plumbing").
9. **Does not block the follow-on spec:** `open_view_source` (`app/navigation.rs:432-441`) keeps its name and single-concept-id ergonomics via a thin wrapper; `SourceToggleView` and `GenericOkfView` internals are untouched; the `GenericOkfView` second in-place flip recorded in `docs/superpowers/plans/drafts/2026-08-08-source-as-navigation.md` (discrepancy 2 there) remains that plan's problem.

## Verified touch points (current worktree, checked 2026-08-08)

| Path | What is there today |
|---|---|
| `crates/waml-editor/src/view_history.rs:18-45` | `DocumentKind` (2 variants), `DocumentLocator { concept_id, kind }`, `::new/primary/source`; `ViewLocation { document, anchor }` `:88-92` |
| `crates/waml-editor/src/navigation.rs:1` | `pub use crate::view_history::{DocumentKind, DocumentLocator}`; `NavigationTarget` `:8-17` |
| `crates/waml-editor/src/documents.rs` | `describe` `.or_else` `:5-12`; `open_with_asset_host` `.or_else` chain `:14-22`; `KNOWN_SURFACES` `:31-32`; `open_row_with_asset_host` `:59-97` (`#[allow(dead_code)]`); `open_folder` `:103-110`; `reopen_with_asset_host` `:121-128`; `open_locator_with_asset_host` + the ONLY `DocumentKind` dispatch `:130-142` |
| `crates/waml-editor/src/extension_editor.rs` | `OpenCtx` `:40-54` (with the `resolve` closure `:49`); `SurfaceFactory` on `&RowId` `:60`; `EditorExtension` `:64-75`; `CoreEditorExtension::surfaces` `:89-95`; `UmlEditorExtension` `:114-128`; bare-view factories `:168-208`; all `#[allow(dead_code)]` |
| `crates/waml-editor/src/okf_documents.rs` | tab ids `:14-20`; `open_with_asset_host` `:47-73`; `open_source_with_asset_host` `:86-111`, concept gate `:91` |
| `crates/waml-editor/src/uml_documents.rs` | `uml_document_tab_id` `:8-10`; claim-gated `describe` `:62-83`; `open_with_asset_host` `:85-136` (wraps in `SourceToggleView`, 4-way category dispatch) |
| `crates/waml-editor/src/folder_documents.rs` | `open` `:41-58` stamps `concept_id: directory, kind: Primary` `:52-53` |
| `crates/waml-editor/src/folder_view.rs:215-217` | `folder_document_tab_id` |
| `crates/waml-editor/src/document.rs:70-99` | `OpenDocument { tab_id, concept_id, kind, ... }`; `locator()` `#[cfg(test)]` `:80-83`; `into_tab` `:85-98` |
| `crates/waml-editor/src/doc_tabs.rs:134-158` | `DocTab { id, concept_id, kind, title, presentation, preview, resolved }`; `locator()` `:154-158` |
| `crates/waml-editor/src/document_host.rs` | `tab_id_for_locator` `:119-125`; `capture_active_location` `:179-187`; `restore_location_with_asset_host` `:203-234`; `reconcile_documents` keys on `tab_id` `:438` |
| `crates/waml-editor/src/app/navigation.rs` | `sync_history_controls` resolves via `open_locator...` `:67-116`; `navigate_with` `Document`/`Directory` arms `:234-347`; `apply_pending_fragment` compares `tab.concept_id` `:349-360`; `transition_document` `:405-425`; `open_view_source` `:432-441`; `transition_to_location` `:443-561`; `traverse_view_history` `:563-590`; `refresh_folder_tabs` filters `NavCategory::Directory` `:705-732` |
| `crates/waml-editor/src/app/actions.rs` | doc-switcher activate via `tab.locator()` `:235-252`; `DocTabsAction::Activate` via `tab.locator()` `:750-767`; other `tab.concept_id` reads `:339`, `:535`, `:850`; `promotion_app` test locator `:1088` |
| `crates/waml-editor/src/app/shell.rs:852` | `tab.concept_id.clone()` feeding the breadcrumb subject |
| `crates/waml-editor/src/class_diagram_view.rs:294-300` | `EditMergeKey { document: DocumentLocator::primary(self.key), ... }`; test literals `:1328,1346,1383` |
| `crates/waml-editor/src/editor_session.rs:495-530` | `#[cfg(test)] apply`/`apply_with_preparer`; sentinel `:515` |
| `crates/waml-editor/src/editor_history.rs` | `EditMergeKey.document: DocumentLocator` (equality-compared only, `:335`) |
| `crates/waml/src/view/surface.rs` | `SurfaceId(pub String)` `:11`; `default_surface` `:26-49`; `resolve_surface` `:56-78` |
| `crates/waml/src/view/row.rs` | `RowTarget` `:160-168` (no `Hash`); `Row { target, surface: Option<SurfaceId>, ... }` `:204-218` |
| `crates/waml/src/view/hide.rs:6-11` | the "presentational, never a permission boundary" invariant |
| `crates/waml/src/source.rs:357-365` | `document_by_concept_id` — pure `format!("{id}.md")` path derivation |
| `crates/waml-editor/src/source_view.rs:144-155` | `resolve_document`: `load::source_for` → `document_by_concept_id` → `catalog.id_for_path` → `markdown_snapshot` |
| `crates/waml/src/analysis.rs:130-140` | `OkfAnalysis { catalog, markdown, bundle, ... }` — no `SourceBundle` field; gate must go through `catalog.id_for_path` (Task 3) |
| Locator-literal test files (Task 4 compile-chase) | `tests/view_history.rs`, `tests/history_integration.rs`, `tests/editor_history.rs`, `src/app/tests/navigation.rs`, `src/app/tests/menus.rs`, `src/editor_session/tests.rs:38,1191,2233`, `src/document_host.rs` tests, `src/doc_tabs.rs` tests |

---

### Task 1: Re-shape the dormant seam — `SurfaceFactory` keys on `RowTarget` and returns `OpenDocument`

The spike's non-negotiable correction (Q2), applied while the seam is still dead so it is pure refactor: `RowId`-keying would make `hide`-hidden concepts unopenable. Also fixes research finding 1 (factories must produce tabs, and must produce the SAME views the live path produces) and deletes `OpenCtx::resolve` (spec §6: wrong as specified — `Chain::resolve` returns `Vec<Row>` with a whole-listing fallback — and unneeded once keys are targets). Everything stays behind the existing `#[allow(dead_code)]` markers; no live behavior changes.

**Files:**
- Modify: `crates/waml-editor/src/extension_editor.rs`
- Test: same file's `mod tests`

**Interfaces:**
- Produces (later tasks consume these exact shapes):
  ```rust
  // extension_editor.rs — replaces :40-54 and :60
  pub struct OpenCtx<'a> {
      pub analysis: &'a OkfAnalysis,
      pub uml: &'a waml::uml::Analysis,        // NEW: canvas factory needs it
      pub assets: SharedMarkdownAssetHost,
      pub limits: ChainLimits,
      pub mode: crate::folder_projection::ViewMode,
      // `resolve` DELETED (spec §6 / spike Q1 friction 2).
  }

  pub type SurfaceFactory =
      Box<dyn Fn(&OpenCtx<'_>, &RowTarget) -> Option<crate::document::OpenDocument>>;
  ```
- Consumes: the four provider functions as they exist today (unchanged signatures).

- [ ] **Step 1: Rewrite the four factory bodies as provider delegates.** Replace `concept_href` and the four free functions (`:168-208`) with target-matching delegates; each factory extracts what it needs from the `RowTarget` and hands the rest to the provider that the LIVE path uses today:
  ```rust
  fn open_markdown(ctx: &OpenCtx<'_>, target: &RowTarget) -> Option<OpenDocument> {
      let RowTarget::Concept(id) = target else { return None };
      crate::okf_documents::open_with_asset_host(ctx.analysis, id, &ctx.assets)
  }

  fn open_source(ctx: &OpenCtx<'_>, target: &RowTarget) -> Option<OpenDocument> {
      let RowTarget::Concept(id) = target else { return None };
      crate::okf_documents::open_source_with_asset_host(ctx.analysis, id, &ctx.assets)
  }

  fn open_canvas(ctx: &OpenCtx<'_>, target: &RowTarget) -> Option<OpenDocument> {
      let RowTarget::Concept(id) = target else { return None };
      crate::uml_documents::open_with_asset_host(ctx.analysis, ctx.uml, id, &ctx.assets)
  }

  fn open_folder(ctx: &OpenCtx<'_>, target: &RowTarget) -> Option<OpenDocument> {
      let RowTarget::Folder(directory) = target else { return None };
      crate::documents::open_folder(ctx.analysis, directory, ctx.limits, ctx.mode)
  }
  ```
  Note `open_source` becomes folder-capable in Task 3 — leave a one-line comment saying so. Imports of `ClassDiagramView`/`FolderView`/`GenericOkfView`/`SourceView`/`RowId`/`Row` that lose their last use go too. Update the module doc comment (`:9-16`): the `Option` return now means "this surface does not apply to this target", and the RowId-resolution caveat is obsolete.
- [ ] **Step 2: Update the module's tests.** `open_markdown_degrades_to_none_for_a_folder_target` (`:322-350`) loses its hand-rolled `resolve` closure and `RowId`; drive the factories with `&RowTarget::Folder("/sales".into())` / `&RowTarget::Concept(...)` directly against a live `OpenCtx` built like the analysis fixture at `:310-320` (add `uml` from `prepared.into_parts()`; keep `assets`, `limits`, `mode`). Add one new assertion per factory: `open_canvas` returns `Some` for a claimed `uml.Class` concept and its `tab_id == uml_document_tab_id(...)` — pinning that the factory produces the SAME tab identity as the live path (spec Testing bullet 1, at the seam level).
- [ ] **Step 3: Gate** (full, per Global constraints). Nothing outside `extension_editor.rs` compiles against the old shapes — verify with `rg "OpenCtx|SurfaceFactory" crates/ --type rust -l` (expect `extension_editor.rs` only; `spike_scratch.rs` exists only on the spike branch, not here).
- [ ] **Step 4: Commit** — `refactor(editor): key SurfaceFactory on RowTarget, return OpenDocument, drop OpenCtx::resolve`.

### Task 2: Fix the folder-tab locator round-trip on today's shapes ⚠ user-visible behavior change

The live bug (spike Q5): `folder_documents::open` stamps `{concept_id: "/shop", kind: Primary}`, `open_locator_with_asset_host` has no folder arm, so folder tabs never resolve — Back/Forward skips them and tab-bar activation dead-ends. Fixing this BEFORE the widening means the type change in Task 4 carries no behavior with it, and this commit's user-visible change (Back/Forward starts stopping ON folder tabs; clicking an inactive folder tab starts working) ships deliberately and alone. A directory address always starts with `/` and a concept id never does (`DirectoryAddress` vs `path.concept_id()` derivation), so the discrimination is sound on today's stringly locator; Task 4 replaces it with the typed `RowTarget::Folder`.

**Files:**
- Modify: `crates/waml-editor/src/documents.rs:121-142` (`reopen_with_asset_host`, `open_locator_with_asset_host`)
- Modify: `crates/waml-editor/src/document_host.rs:203-253` (`restore_location_with_asset_host` + test wrapper)
- Modify: `crates/waml-editor/src/app/navigation.rs` (`sync_history_controls` `:67-116`, `transition_to_location` `:479-489`, `traverse_view_history` `:563-590`)
- Test: `crates/waml-editor/src/documents.rs` tests; `crates/waml-editor/src/app/tests/navigation.rs`

**Interfaces:**
- Produces: `open_locator_with_asset_host(okf, uml, locator, assets, limits: waml::view::chain::ChainLimits, mode: crate::folder_projection::ViewMode) -> Option<OpenDocument>` — two NEW trailing parameters; same for `reopen_with_asset_host` and `DocumentHost::restore_location_with_asset_host`. Task 4 keeps these signatures (only the locator type changes).
- Consumes: `documents::open_folder` (`:103-110`), `App`'s `chain_limits`/`view_mode` fields.

- [ ] **Step 1: Write the failing test at the documents level** (in `documents.rs`'s test module — the folder fixture from `a_folder_row_opens_through_open_folder` `:379-398` has the shape needed):
  ```rust
  #[test]
  fn a_folder_tabs_locator_reopens_the_folder_view() {
      let source = SourceBundle::try_from_pairs([
          ("index.md", "# Root\n\n* [Sales](sales/)\n"),
          ("sales/index.md", "# Sales\n"),
      ])
      .unwrap();
      let prepared = waml::analysis::prepare_candidate(source, None, 21).unwrap();
      let (folder_tab, _) = open_folder(
          prepared.okf(),
          "/sales",
          waml::view::chain::ChainLimits::default(),
          crate::folder_projection::ViewMode::Projected,
      )
      .unwrap()
      .into_tab(true);

      let reopened = reopen_with_asset_host(
          prepared.okf(),
          prepared.uml(),
          &folder_tab,
          &assets(),
          waml::view::chain::ChainLimits::default(),
          crate::folder_projection::ViewMode::Projected,
      )
      .expect("a folder tab's locator must resolve (spike Q5: today it never does)");
      assert_eq!(reopened.tab_id, folder_tab.id);
  }
  ```
  Run: `cargo test -p waml-editor a_folder_tabs_locator_reopens` — expect FAIL to compile (new params) — add the params first with all existing call sites updated mechanically (Step 2), then expect FAIL on the `expect` (returns `None`).
- [ ] **Step 2: Plumb `limits` + `mode`.** Add the two parameters to `open_locator_with_asset_host`, `reopen_with_asset_host`, and `DocumentHost::restore_location_with_asset_host` (the `#[cfg(test)] restore_location` wrapper passes `ChainLimits::default()` / `ViewMode::Projected`). Fix every caller: `sync_history_controls` (`app/navigation.rs:72,84` closures), `transition_to_location` (`:481`), `traverse_view_history` (`:572`) — all pass `self.chain_limits, self.view_mode`. `reopen_with_asset_host`'s callers: check with `rg "reopen_with_asset_host" crates/` and thread the values from whatever `App`/session context each has (session-prepare paths pass the App's fields; test callers pass defaults).
- [ ] **Step 3: Add the folder arm.** In `open_locator_with_asset_host`:
  ```rust
  match locator.kind {
      // A directory address always begins with '/'; a concept id never does.
      // Temporary discrimination on the stringly locator -- replaced by the
      // typed RowTarget::Folder arm when the locator widens (surface plan §1).
      DocumentKind::Primary if locator.concept_id.starts_with('/') => {
          open_folder(okf, &locator.concept_id, limits, mode)
      }
      DocumentKind::Primary => open_with_asset_host(okf, uml, &locator.concept_id, assets),
      DocumentKind::Source => { ... unchanged ... }
  }
  ```
  Run the Step 1 test: PASS.
- [ ] **Step 4: Write the app-level history test** (in `app/tests/navigation.rs`, using that file's existing app-builder + history helpers — the tests around `:761-965` show the pattern of driving `transition_to_location` and asserting `active_tab()`; a `Directory` navigation goes through `navigate_with(... NavigationTarget::Directory { address }, ...)` or the same helper those tests use for folder opens around `:1690-1726`): open concept `sales/order` (persistent), navigate to directory `/sales`, navigate to concept `sales/customer`; drive history Back once; **assert the active tab is the `/sales` folder tab** (today Back skips it and lands on `sales/order`); Back again lands on `sales/order`; Forward returns to `/sales`. Also assert `app.documents.tab_id_for_locator(&folder_tab_locator)` finds the tab. Name it `back_and_forward_stop_on_folder_tabs`.
- [ ] **Step 5: Gate, then commit** — `fix(editor): folder tab locators resolve; Back/Forward stops on folder tabs` — with a body line stating the deliberate user-visible change.

### Task 3: The `"source"` surface resolves per target, with the render-predicate gate

Spec §5 with the spike's Q3 correction baked in: the folder gate is the same predicate that decides whether `SourceView` can render — NOT `Bundle::index`, which synthesizes an `Index` for directories with no `index.md` on disk. Pure functions plus the factory wiring; nothing reachable from the UI yet (the locator cannot say "folder + source" until Task 4), so no behavior change.

**Files:**
- Modify: `crates/waml-editor/src/okf_documents.rs` (new functions beside `open_source_with_asset_host`)
- Modify: `crates/waml-editor/src/extension_editor.rs` (the `"source"` factory body from Task 1)
- Test: `okf_documents.rs` tests

**Interfaces:**
- Produces:
  ```rust
  // okf_documents.rs
  /// The source-document key for a target: a concept is its own key; a
  /// folder's key is its index document ("/shop" -> "shop/index", "/" ->
  /// "index"); a Virtual target has no source.
  pub fn source_key_for(target: &waml::view::row::RowTarget) -> Option<String>;

  /// The render predicate: does a real markdown document exist at `key`?
  /// The SAME resolution SourceView::resolve_document performs
  /// (source_view.rs:144-155) minus the snapshot: path derivation ->
  /// catalog -> markdown analysis. NOT Bundle::index, which synthesizes
  /// (spike Q3 refutation).
  pub fn source_document_exists(analysis: &waml::analysis::OkfAnalysis, key: &str) -> bool;

  /// Target-resolving source open (spec §5). Concept targets behave exactly
  /// as open_source_with_asset_host; Folder targets open the index document.
  pub fn open_source_for_target(
      analysis: &waml::analysis::OkfAnalysis,
      target: &waml::view::row::RowTarget,
      assets: &crate::markdown_hosts::SharedMarkdownAssetHost,
  ) -> Option<OpenDocument>;
  ```
- Consumes: Task 1's factory shapes.

- [ ] **Step 1: Write the failing tests first** (in `okf_documents.rs`'s test module):
  ```rust
  #[test]
  fn folder_source_resolves_through_the_index_key_and_the_root_works() {
      let source = SourceBundle::try_from_pairs([
          ("index.md", "# Root\n\n* [Shop](shop/)\n"),
          ("shop/index.md", "# Shop\n"),
      ])
      .unwrap();
      let prepared = waml::analysis::prepare_candidate(source, None, 31).unwrap();
      use waml::view::row::RowTarget;
      assert_eq!(source_key_for(&RowTarget::Folder("/shop".into())).as_deref(), Some("shop/index"));
      assert_eq!(source_key_for(&RowTarget::Folder("/".into())).as_deref(), Some("index"));
      assert_eq!(source_key_for(&RowTarget::Concept("shop/thing".into())).as_deref(), Some("shop/thing"));
      assert_eq!(source_key_for(&RowTarget::Virtual), None);
      let doc = open_source_for_target(prepared.okf(), &RowTarget::Folder("/shop".into()), &assets())
          .expect("a folder with an index.md resolves the source surface");
      assert_eq!(doc.tab_id, source_document_tab_id("shop/index"));
      assert!(open_source_for_target(prepared.okf(), &RowTarget::Folder("/".into()), &assets()).is_some());
  }

  /// The test the spec's own last Testing bullet demands, and the one that
  /// FAILS if the gate is Bundle::index (spike Q3): /loose has concepts but
  /// no index.md on disk; Bundle::index still answers Some (synthesized),
  /// while the source surface must not resolve.
  #[test]
  fn a_folder_without_an_index_md_does_not_resolve_source_even_though_bundle_index_answers() {
      let source = SourceBundle::try_from_pairs([
          ("index.md", "# Root\n\n* [Loose](loose/)\n"),
          ("loose/thing.md", "---\ntype: Runbook\n---\n# Thing\n"),
      ])
      .unwrap();
      let prepared = waml::analysis::prepare_candidate(source, None, 33).unwrap();
      assert!(prepared.okf().bundle.index("/loose").is_some(), "the wrong gate would say yes here");
      assert!(!source_document_exists(prepared.okf(), "loose/index"));
      assert!(open_source_for_target(
          prepared.okf(),
          &waml::view::row::RowTarget::Folder("/loose".into()),
          &assets()
      )
      .is_none());
  }
  ```
  Run: expect FAIL (functions do not exist). *(Fixture check: verify `/loose` actually enters the bundle as a directory when only linked as `loose/` with no index — if `prepare_candidate` drops the directory instead, link `loose/thing.md` directly from the root index and re-check `bundle.index("/loose")`; the test's point is the divergence between `bundle.index` and the render predicate, and the spike proved that divergence against a bare directory.)*
- [ ] **Step 2: Implement.** `source_key_for`: `Concept(id)` → `Some(id.clone())`; `Folder(addr)` → `Some(if addr == "/" { "index".into() } else { format!("{}/index", addr.trim_start_matches('/')) })`; `Virtual` → `None`. `source_document_exists`: parse `BundlePath` from `format!("{key}.md")`, `analysis.catalog.id_for_path(&path)`, then `analysis.markdown_snapshot(id).is_some()` (mirrors `resolve_document`'s chain; `OkfAnalysis` has no `SourceBundle`, and the catalog is derived from the real source documents, so catalog presence == file-on-disk). `open_source_for_target`: `let key = source_key_for(target)?;` gate on `source_document_exists`; for `Concept`, delegate to `open_source_with_asset_host` unchanged (its concept gate + title stand); for `Folder`, build the `OpenDocument` directly — `tab_id: source_document_tab_id(&key)`, `concept_id: key.clone()` *(the DocTab still carries strings until Task 4; Task 4 re-stamps this as `{target: Folder(addr), surface: "source"}`)*, `kind: DocumentKind::Source`, title from the folder (reuse `folder_documents`-style resolution: `analysis.bundle.index(addr).and_then(|i| i.title.clone())` falling back to the address's last segment — using `index` for a TITLE is fine, it is only the gate it must not be), presentation `{ icon: Icon::FileCode, accent: generic_okf_accent(), category: NavCategory::OkfDocument }`, view `SourceView::new_with_asset_host(key, assets.clone())`.
- [ ] **Step 3: Wire the `"source"` factory** (`extension_editor.rs`) to `open_source_for_target(ctx.analysis, target, &ctx.assets)` and delete its `Concept`-only guard. Extend the factory test from Task 1 Step 2: the `"source"` factory returns `Some` for a Folder target with an index and `None` for one without.
- [ ] **Step 4: Gate, commit** — `feat(editor): target-resolving source surface with the render-predicate folder gate`.

### Task 3b: Move the call sites to surface vocabulary BEFORE the type changes

Prep task, added 2026-08-08 to shrink Task 4's atomic surface. Verified against the worktree first: `DocumentLocator` has **zero** struct-literal construction sites outside its own definition (all ~41 sites already go through `::new`/`::primary`/`::source`) and only ~3 locator field reads. So the churn in Task 4 is not field access — it is the ~41 constructor calls and the 34 `DocumentKind` mentions. This task migrates the call sites onto the FINAL vocabulary while the underlying type is still today's `{concept_id, kind}`, so Task 4 reshapes internals against call sites that already read correctly. Everything here compiles and gates green on its own; no behavior changes.

**Files:**
- Modify: `crates/waml/src/view/row.rs:160` (add `Hash` to `RowTarget`'s derive)
- Modify: `crates/waml/src/view/surface.rs` (named constructors)
- Modify: `crates/waml-editor/src/view_history.rs` (transitional `concept` constructor)
- Modify: `crates/waml-editor/src/documents.rs` (`default_surface_for`), `app/navigation.rs` (`primary_locator`)
- Modify: every `DocumentLocator::primary(...)` call site, production and test

**Interfaces:**
- Produces the final `SurfaceId::{markdown,source,canvas,folder}()` constructors and `documents::default_surface_for` / `App::primary_locator` EXACTLY as Task 4's `Interfaces` block specifies them — copy those bodies verbatim; they do not depend on the widened locator.
- Produces one TRANSITIONAL constructor on today's struct, deleted by Task 4 Step 1:
  ```rust
  // view_history.rs — surface vocabulary over today's kind. Task 4 replaces the
  // body when the struct widens; the call sites it serves do not change again.
  impl DocumentLocator {
      pub fn concept(concept_id: impl Into<String>, surface: SurfaceId) -> Self {
          let kind = if surface == SurfaceId::source() {
              DocumentKind::Source
          } else {
              DocumentKind::Primary
          };
          Self::new(concept_id, kind)
      }
  }
  ```
  `markdown` and `canvas` both map to `Primary` — that IS today's behavior (the `.or_else` chain picks the provider), which is why this commit is behavior-identical.

- [ ] **Step 1: Land the additive pieces.** `Hash` on `RowTarget`; `SurfaceId` named constructors; the transitional `concept` constructor. Gate — nothing else has changed yet.
- [ ] **Step 2: Add the default-surface resolution.** `documents::default_surface_for` and `App::primary_locator` per Task 4's Interfaces block, with `primary_locator` returning `DocumentLocator::concept(concept_id, default_surface_for(...))`. Unit-test `default_surface_for` directly: a claimed `uml.Class` concept → `canvas()`, a generic concept → `markdown()`, a folder → `folder()`.
- [ ] **Step 3: Migrate the call sites off `primary`.** Production sites per Task 4 Step 5's inventory — `app/navigation.rs:275,414` → `self.primary_locator(&concept_id)`; `class_diagram_view.rs:296` → `DocumentLocator::concept(self.key.clone(), SurfaceId::canvas())`. Test sites → `DocumentLocator::concept(x, SurfaceId::markdown())` for generic fixtures, `SurfaceId::canvas()` where the fixture is a claimed `uml.*` (`editor_session/tests.rs:1191,2233` use `"dia"` diagrams). Then **delete `DocumentLocator::primary`** — its absence is what proves the migration is complete. `DocumentLocator::source` keeps its name and body through Task 4; leave those sites alone.
- [ ] **Step 4: Sweep and gate.** `rg "DocumentLocator::primary" crates/` → zero hits. Full gate. Every pre-existing test must PASS, not merely compile — a failure here is a real surface-classification mistake (most likely a fixture assigned `markdown()` that the UML analysis actually claims), so fix the mapping, not the test.
- [ ] **Step 5: Commit** — `refactor(editor): express locators in surface vocabulary ahead of the widening`.

### Task 4: Widen `DocumentLocator` to `{ target, surface }` and delete `DocumentKind`

The load-bearing recompile. **Task 3b has already landed** the `Hash` derive, the `SurfaceId` constructors, `default_surface_for`, `primary_locator`, and the `::primary` → `::concept(id, surface)` call-site migration — do not redo them; this task deletes the transitional `concept` body and reshapes the struct beneath call sites that already read correctly. Task 4 Step 1 additionally deletes the transitional constructor's kind-mapping; Steps 2–8 stand as written, minus any work Task 3b already did. Spike Q4: 36 `DocumentKind` mentions in 13 files, ~41 construction sites, exactly one dispatching match, zero persistence — wide, shallow, compiler-guided. This task changes SHAPE only: dispatch stays behavior-identical (the surface string routes to the same provider today's kind routed to, with the concept default still resolved by the `.or_else` chain — Task 5 replaces the mechanism). Tab-id functions are untouched (Task 7). No user-visible change is expected from this commit; the folder arm from Task 2 becomes typed.

This task is file-heavy by nature and cannot be split at a compiling boundary — a type change is atomic. It is kept tractable by being almost entirely mechanical (the compiler drives), by Tasks 1–3 having already landed every non-mechanical decision, and by the site inventory below. Budget accordingly; do not interleave any other refactor.

**Files:**
- Modify: `crates/waml/src/view/row.rs:160` (add `Hash` to `RowTarget`'s derive)
- Modify: `crates/waml/src/view/surface.rs` (named constructors)
- Modify: `crates/waml-editor/src/view_history.rs:18-45` (the locator itself)
- Modify: `crates/waml-editor/src/navigation.rs:1` (re-export: drop `DocumentKind`)
- Modify: `crates/waml-editor/src/document.rs`, `doc_tabs.rs`, `documents.rs`, `document_host.rs`, `okf_documents.rs`, `uml_documents.rs`, `folder_documents.rs`, `class_diagram_view.rs`, `editor_session.rs:515`, `app/navigation.rs`, `app/actions.rs`, `app/shell.rs:852`
- Modify (tests): every file in the touch-point table's last row

**Interfaces:**
- Produces:
  ```rust
  // waml: surface.rs — named constructors for the four registered ids
  impl SurfaceId {
      pub fn markdown() -> Self { SurfaceId("markdown".into()) }
      pub fn source() -> Self { SurfaceId("source".into()) }
      pub fn canvas() -> Self { SurfaceId("canvas".into()) }
      pub fn folder() -> Self { SurfaceId("folder".into()) }
  }

  // view_history.rs — replaces :18-45
  use waml::view::row::RowTarget;
  use waml::view::surface::SurfaceId;

  #[derive(Clone, Debug, PartialEq, Eq, Hash)]
  pub struct DocumentLocator {
      pub target: RowTarget,
      pub surface: SurfaceId,
  }

  impl DocumentLocator {
      pub fn new(target: RowTarget, surface: SurfaceId) -> Self { Self { target, surface } }
      /// A concept on an explicit surface.
      pub fn concept(concept_id: impl Into<String>, surface: SurfaceId) -> Self {
          Self::new(RowTarget::Concept(concept_id.into()), surface)
      }
      /// A folder's own listing surface.
      pub fn folder(address: impl Into<String>) -> Self {
          Self::new(RowTarget::Folder(address.into()), SurfaceId::folder())
      }
      /// A concept's raw-markdown surface.
      pub fn source(concept_id: impl Into<String>) -> Self {
          Self::concept(concept_id, SurfaceId::source())
      }
      /// The concept id, when this locator names a concept.
      pub fn concept_id(&self) -> Option<&str> {
          match &self.target { RowTarget::Concept(id) => Some(id), _ => None }
      }
  }
  ```
  `DocumentLocator::primary` is DELETED, not re-expressed — "primary" is a resolution, not a surface (spec §1). Its replacement at call sites is `App::primary_locator` below or an explicit surface the site actually knows.
  ```rust
  // documents.rs — the editor's default resolution (spec §1/§4; research finding 5)
  /// The surface a target opens on when nothing requests one. "canvas" iff
  /// the UML analysis claims the concept (the claim set is what decides
  /// provider ownership today -- NOT ElementType parsing, which can disagree
  /// on invalid-but-claimed documents); "markdown" otherwise; "folder" for a
  /// folder. Virtual has no default (Row::new enforces an explicit surface).
  pub fn default_surface_for(
      okf: &waml::analysis::OkfAnalysis,
      uml: &waml::uml::Analysis,
      target: &waml::view::row::RowTarget,
  ) -> waml::view::surface::SurfaceId {
      use waml::view::row::RowTarget;
      use waml::view::surface::SurfaceId;
      match target {
          RowTarget::Folder(_) => SurfaceId::folder(),
          RowTarget::Concept(id) if uml.claims.contains(id) => SurfaceId::canvas(),
          RowTarget::Concept(_) | RowTarget::Virtual => SurfaceId::markdown(),
      }
  }

  // app (App impl, app/navigation.rs) — the "primary" resolution at click sites
  pub(super) fn primary_locator(&self, concept_id: &str) -> crate::navigation::DocumentLocator {
      let target = waml::view::row::RowTarget::Concept(concept_id.to_string());
      let surface = crate::documents::default_surface_for(
          self.session.okf_analysis(),
          self.session.uml_analysis(),
          &target,
      );
      crate::navigation::DocumentLocator::new(target, surface)
  }
  ```
  `OpenDocument` and `DocTab` each replace their `{ concept_id: String, kind: DocumentKind }` pair with `locator: DocumentLocator` and grow `pub fn concept_id(&self) -> Option<&str>` delegating to the locator; `DocTab::locator()` / `OpenDocument::locator()` return `self.locator.clone()` (drop `#[cfg(test)]` on the latter — it becomes genuinely shared).
- Consumes: Task 2's plumbed `limits`/`mode`; Task 3's `open_source_for_target`.

- [ ] **Step 1: Widen the types.** Apply the `Interfaces` block: `RowTarget` gains `Hash`; `SurfaceId` constructors; `DocumentLocator` new shape; `navigation.rs:1` re-export becomes `pub use crate::view_history::DocumentLocator;` (the `DocumentKind` name disappears from the crate). Delete the `DocumentKind` enum. From here the crate does not compile until Step 5 — that is the plan; work through the clusters in order, `cargo check -p waml-editor` between clusters to keep the error list shrinking.
- [ ] **Step 2: Providers stamp honest locators.** `okf_documents::open_with_asset_host` → `locator: DocumentLocator::concept(concept_id, SurfaceId::markdown())`; `open_source_with_asset_host` → `::source(concept_id)`; the folder half of `open_source_for_target` → `DocumentLocator::new(RowTarget::Folder(address), SurfaceId::source())` (keep deriving its tab id / view key from the `shop/index` key — pass the address through so the locator gets the FOLDER, not the key: adjust `open_source_for_target` to build the locator from its `target` argument directly); `uml_documents::open_with_asset_host` → `::concept(concept_id, SurfaceId::canvas())`; `folder_documents::open` → `::folder(directory)` — **the lie at `folder_documents.rs:52-53` dies here.** `OpenDocument::into_tab` copies the locator.
- [ ] **Step 3: Dispatch on the surface, behavior-identically.** `open_locator_with_asset_host` (`documents.rs:130-142`) becomes:
  ```rust
  pub fn open_locator_with_asset_host(
      okf: &waml::analysis::OkfAnalysis,
      uml: &waml::uml::Analysis,
      locator: &DocumentLocator,
      assets: &crate::markdown_hosts::SharedMarkdownAssetHost,
      limits: waml::view::chain::ChainLimits,
      mode: crate::folder_projection::ViewMode,
  ) -> Option<OpenDocument> {
      use waml::view::row::RowTarget;
      match (locator.surface.as_str(), &locator.target) {
          ("folder", RowTarget::Folder(directory)) => open_folder(okf, directory, limits, mode),
          ("source", target) => crate::okf_documents::open_source_for_target(okf, target, assets),
          ("canvas", RowTarget::Concept(id)) => {
              // A stored canvas locator can go stale when a concept's type is
              // edited away from uml.*; today's Primary arm fell through to
              // the generic provider, so the canvas surface keeps that
              // degrade path (research finding 5). Task 5 keeps this arm.
              crate::uml_documents::open_with_asset_host(okf, uml, id, assets)
                  .or_else(|| crate::okf_documents::open_with_asset_host(okf, id, assets))
          }
          (_, RowTarget::Concept(id)) => crate::okf_documents::open_with_asset_host(okf, id, assets),
          _ => None,
      }
  }
  ```
  Delete Task 2's `starts_with('/')` discrimination (superseded by the typed arm). NOTE the top-level `open_with_asset_host` `.or_else` chain (`:14-22`) still has non-locator callers after this step (e.g. session prepare via `reopen`? — check with `rg "open_with_asset_host\(" crates/waml-editor/src`); leave it and `describe` untouched here.
- [ ] **Step 4: The sentinel.** `editor_session.rs:515` becomes:
  ```rust
  let location = ViewLocation {
      // A test-helper edit with no document behind it: Virtual is the honest
      // target (spec §1 -- a Virtual locator exists but nothing opens it),
      // and equality is all EditMergeKey/history need from it.
      document: DocumentLocator::new(
          waml::view::row::RowTarget::Virtual,
          waml::view::surface::SurfaceId::markdown(),
      ),
      anchor: ViewAnchor::None,
  };
  ```
- [ ] **Step 5: Compile-chase the consumers, cluster by cluster.** The inventory (every non-test site, verified):
  - `app/navigation.rs:275` (`Document` arm), `:414` (`transition_document`) → `self.primary_locator(&concept_id)`. `:214` (`navigate_to_source_range`) and `:436` (`open_view_source`) → `DocumentLocator::source(...)` unchanged in name. `apply_pending_fragment` `:356` → `tab.concept_id() != Some(pending.concept_id.as_str())` (typed: a folder tab can no longer collide with a concept fragment — the spike's "type-level accident" is closed). `refresh_folder_tabs` `:705-732` → filter `matches!(tab.locator.target, RowTarget::Folder(_))` and read the address from the target instead of `tab.concept_id` (drop the `NavCategory::Directory` proxy).
  - `class_diagram_view.rs:296` merge key → `DocumentLocator::concept(self.key.clone(), SurfaceId::canvas())` — the diagram view IS the canvas surface of its key; no bundle lookup at edit time. Mirror in its tests `:1328,1346,1383`.
  - `app/actions.rs:339`, `:535`, `:850`, `app/shell.rs:852` → `tab.concept_id()` (`Option`-aware; `:850` reads `location.document` — use `concept_id()` with a sensible fallback such as the folder address via a small `DocumentLocator::subject(&self) -> &str` helper ONLY if the compiler shows a site that genuinely needs a display string for any target; prefer per-site handling). Doc-switcher `:241` and `DocTabsAction::Activate` `:756` use `tab.locator()` — unchanged.
  - `document.rs:80-98`, `doc_tabs.rs:134-158` — the struct edits from Interfaces.
  - `document_host.rs` — `tab_id_for_locator`, `capture_active_location`, `restore_location_with_asset_host` compile as-is once `DocTab::locator()` returns the new type.
  - Tests: mechanical rewrites. `DocumentLocator::primary(x)` in tests becomes `DocumentLocator::concept(x, SurfaceId::markdown())` where the fixture concept is generic (most), `SurfaceId::canvas()` where it is a claimed `uml.*` (check each fixture's frontmatter — `app/tests/navigation.rs` fixtures are mostly generic; `editor_session/tests.rs:1191,2233` use `"dia"` diagrams → canvas). `tab.kind == DocumentKind::Source` assertions become `tab.locator.surface == SurfaceId::source()`. `document_host.rs`/`doc_tabs.rs` test builders that set `kind`/`concept_id` set `locator` instead.
- [ ] **Step 6: Sweep.** `rg "DocumentKind" crates/` → zero hits. `rg "__legacy_edit__" crates/` → zero hits. `cargo check -p waml-editor` clean.
- [ ] **Step 7: Full gate.** Every pre-existing test passes with only the mechanical rewrites above — any test that FAILS (rather than fails-to-compile) is a behavior regression this task must not have; stop and fix the dispatch, not the test. Watch: `sync_history_controls` and `traverse_view_history` closures now resolve folder locators through the typed arm — the Task 2 app test must still pass.
- [ ] **Step 8: Commit** — `refactor(editor): DocumentLocator = { RowTarget, SurfaceId }; delete DocumentKind`.

### Task 5: The registry becomes the open path; the `hide` invariant enters the test suite

Wiring, deliberately split from Task 4's file-heavy shape change. `open_locator_with_asset_host`'s hand-written surface match becomes a lookup through `EditorExtension::surfaces()`, the dormant seam's `#[allow(dead_code)]` blanket comes off, and the two regression tests the spec demands (hidden concepts still open; unknown surface degrades on a LIVE path) land.

**Files:**
- Modify: `crates/waml-editor/src/documents.rs`
- Modify: `crates/waml-editor/src/extension_editor.rs` (strip allows; add a registry accessor)
- Test: `documents.rs` tests

**Interfaces:**
- Produces:
  ```rust
  // extension_editor.rs
  /// The surface table an editor build actually registers -- core + uml, the
  /// same pair as folder_projection::core_registry's middleware side.
  pub fn surface_table() -> Vec<(&'static str, SurfaceFactory)> {
      let mut table = CoreEditorExtension.surfaces();
      table.extend(UmlEditorExtension.surfaces());
      table
  }
  ```
  `documents::open_locator_with_asset_host` builds an `OpenCtx { analysis: okf, uml, assets: assets.clone(), limits, mode }`, resolves `locator.surface` against the table, and calls the matching factory with `&locator.target`. The `"canvas"` factory itself absorbs the degrade path (`uml_documents::open...().or_else(okf_documents::open...)` moves INTO `open_canvas`, with the research-finding-5 comment). Unknown surface id in a stored locator → `resolve_surface(Some(id), &target, &okf.bundle, KNOWN_SURFACES, ...)` degrade → the resolved factory (never a blank tab).
- Consumes: Task 4's dispatch (replaced), Task 1's factory shapes, `default_surface_for` (Task 4).

- [ ] **Step 1: Write the hide-invariant regression test FIRST** (`documents.rs` tests) — this is the regression the `RowTarget` re-keying exists to prevent, pinned per the spec's Risks section against `crates/waml/src/view/hide.rs:7`:
  ```rust
  /// `hide` is presentational, never a permission boundary (hide.rs:7). A
  /// concept a middleware hides has NO RowId in Projected mode (spike Q2),
  /// so any open path keyed on row identity would make it unopenable. The
  /// locator path must open it in BOTH modes.
  #[test]
  fn a_hidden_concept_still_opens_through_the_surface_path_in_both_modes() {
      let source = SourceBundle::try_from_pairs([
          ("index.md", "# Root\n\n* [Shop](shop/)\n"),
          (
              "shop/index.md",
              "---\nview: hide\nhide: [\"shop/secret\"]\n---\n# Shop\n\n* [Order](order.md)\n* [Secret](secret.md)\n",
          ),
          ("shop/order.md", "---\ntype: Runbook\n---\n# Order\n"),
          ("shop/secret.md", "---\ntype: Runbook\n---\n# Secret\n"),
      ])
      .unwrap();
      let prepared = waml::analysis::prepare_candidate(source, None, 41).unwrap();
      let locator = DocumentLocator::concept("shop/secret", waml::view::surface::SurfaceId::markdown());
      for mode in [
          crate::folder_projection::ViewMode::Projected,
          crate::folder_projection::ViewMode::Raw,
      ] {
          assert!(
              open_locator_with_asset_host(
                  prepared.okf(),
                  prepared.uml(),
                  &locator,
                  &assets(),
                  waml::view::chain::ChainLimits::default(),
                  mode,
              )
              .is_some(),
              "hide must stay presentational in {mode:?}"
          );
      }
      // The source surface of a hidden concept opens too.
      assert!(open_locator_with_asset_host(
          prepared.okf(), prepared.uml(),
          &DocumentLocator::source("shop/secret"),
          &assets(), waml::view::chain::ChainLimits::default(),
          crate::folder_projection::ViewMode::Projected,
      ).is_some());
  }
  ```
  Run: expect PASS already (Task 4's dispatch never consults rows) — this is a pin, not a repro; it exists to fail if anyone reintroduces row-relative identity. Then proceed.
- [ ] **Step 2: Route through the table.** Rewrite `open_locator_with_asset_host` per Interfaces; move the canvas degrade into `open_canvas`; delete the now-redundant match arms. `open_row_with_asset_host` (`documents.rs:59-97`) collapses onto the same helper: resolve `row.surface` override via `resolve_surface` (unchanged), then the same table lookup — one open function, two entry shapes. Its doc comment's "not yet called from the live path" paragraph (`:49-57`) is now stale on the locator side — rewrite it to say the locator path IS live through the table and only the row-click path still routes via `NavigationTarget` (Task 6).
- [ ] **Step 3: Strip the allows.** Remove `#[allow(dead_code)]` from: `OpenCtx`, `SurfaceFactory`, `EditorExtension`, `CoreEditorExtension`, `UmlEditorExtension`, the four factory fns (`extension_editor.rs`), `KNOWN_SURFACES` + `open_row_with_asset_host` (`documents.rs:31,58`). `resolve_icon` (`extension_editor.rs:137`) is icon-side and stays allow'd if still unwired — do not touch it. Anything the compiler then flags dead must gain a live caller or be deleted, never re-allowed.
- [ ] **Step 4: Unknown-surface degrade on the live path.** Test: a locator hand-built with `SurfaceId("no-such-surface".into())` for a generic concept opens the markdown tab (degrades via `resolve_surface`'s default rather than returning `None`); assert the same tab id as the plain markdown open. (The existing `an_unknown_surface_override_degrades_with_a_diagnostic` test covers the row entry; this covers the locator entry.)
- [ ] **Step 5: Behavior-preservation check.** Existing `documents.rs` tests (`uml_provider_precedes_generic_okf_provider`, `locator_reopens_the_correct_view_after_transient_tab_identity_is_gone`) must pass UNMODIFIED from Task 4's versions — they are the spec's "every pair that today produces a tab still produces the same tab" bullet.
- [ ] **Step 6: Gate, commit** — `feat(editor): route document opens through the surface table; pin the hide invariant`.

### Task 6: `NavigationTarget` carries the surface; view-source generalizes to targets

Spec §2: the field whose absence `documents.rs:49-57` named as the blocker. After this task a click (or any programmatic navigation) can request an explicit surface end to end, and `open_view_source` works for folder targets — the affordance that USES it on folder tabs belongs to the follow-on spec, so the only new producers here are tests and the existing concept-side callers.

**Files:**
- Modify: `crates/waml-editor/src/navigation.rs:8-17` (`NavigationTarget::Document`)
- Modify: `crates/waml-editor/src/app/navigation.rs` (`navigate_with` `Document` arm `:242-292`; `open_view_source` `:432-441`)
- Modify: producers of `NavigationTarget::Document` (compile-chase: `navigation.rs` `resolve_link`/`breadcrumb_for` `:113,221,286`, plus every struct-literal site `rg "NavigationTarget::Document" crates/` finds — all gain `surface: None`)
- Test: `app/tests/navigation.rs`, `navigation.rs` tests

**Interfaces:**
- Produces:
  ```rust
  NavigationTarget::Document {
      concept_id: String,
      /// None = the target's default resolution (documents::default_surface_for).
      surface: Option<waml::view::surface::SurfaceId>,
      fragment: Option<String>,
  }

  // app/navigation.rs — replaces open_view_source's body; same public shape
  // plus a target-shaped sibling the follow-on spec (and folder affordances)
  // will call:
  pub(super) fn open_view_source(&mut self, cx: &mut Cx, key: &str) { /* delegates */ }
  pub(super) fn open_source_for(&mut self, cx: &mut Cx, target: waml::view::row::RowTarget) {
      self.transition_to_location(
          cx,
          ViewLocation {
              document: crate::navigation::DocumentLocator::new(
                  target,
                  waml::view::surface::SurfaceId::source(),
              ),
              anchor: ViewAnchor::None,
          },
          TransitionCause::UserNavigation,
      );
  }
  ```
- Consumes: Task 4's locator, Task 5's routed open path.

- [ ] **Step 1: Add the field.** Widen the enum; every existing constructor site gains `surface: None` (mechanical; `resolve_link` and breadcrumbs never request one). In `navigate_with`'s `Document` arm (`:272-279`), the locator becomes: `surface` requested → `resolve_surface(Some(id.as_str()), &target, &bundle, documents::KNOWN_SURFACES, ...)` then `DocumentLocator::new(target, resolved)` (degrade semantics for free, make `KNOWN_SURFACES` `pub(crate)`); `None` → `self.primary_locator(&concept_id)` as today. Keep the arm's existing concept-existence precheck (`:246-251`) for the default path; an explicit `"source"` request must NOT be blocked by it when the target is index-like — actually the `Document` arm's `concept_id` is always a concept here; folder-source arrives via `open_source_for`, so the precheck stands unchanged.
- [ ] **Step 2: Failing test — an explicit surface survives a navigation.** In `app/tests/navigation.rs`: `navigate_with(NavigationTarget::Document { concept_id: "sales/order", surface: Some(SurfaceId::source()), fragment: None }, Preview)` on a fixture app; assert the active tab's locator is `DocumentLocator::source("sales/order")` and its tab id equals the one `open_view_source` produces for the same key — **the duplication this spec exists to remove, asserted as identity** (spec Testing bullet 4). Write, watch fail (field/dispatch not there mid-step), implement, pass.
- [ ] **Step 3: Generalize view-source.** Implement `open_source_for` per Interfaces; `open_view_source(key)` becomes `self.open_source_for(cx, RowTarget::Concept(key.to_string()))` — every existing caller (node context menu, behavior canvas via `apply_view_outcome`) compiles untouched.
- [ ] **Step 4: Gate, commit** — `feat(editor): NavigationTarget carries a surface; view-source takes any target`.

### Task 7: One tab-id function over `(target, surface)`

Spec §3: the four namespaces (`__doc_tab_okf__`, `__doc_tab_source__`, `__doc_tab_uml__`, `__doc_tab_folder__`) become one derivation from the locator. `LiveId` values change; nothing is persisted (spike Q4), and reconciliation (`document_host.rs:438`) compares prepared-vs-current ids that are BOTH minted by the new function, so identity semantics hold. Distinctness — two surfaces of one concept are two tabs; a folder never collides with a like-named concept — is preserved by construction and pinned by test.

**Files:**
- Modify: `crates/waml-editor/src/documents.rs` (new `tab_id_for`)
- Modify: `crates/waml-editor/src/okf_documents.rs:14-20`, `uml_documents.rs:8-10`, `folder_view.rs:215-217` (delete the four fns), providers to call `tab_id_for`
- Test: `okf_documents.rs` / `documents.rs` tests

**Interfaces:**
- Produces:
  ```rust
  // documents.rs
  /// THE tab identity: one function over the locator (spec §3). The target
  /// discriminant is baked into the string so a folder "/x" and a concept
  /// "x" can never collide, and two surfaces of one target stay two tabs.
  pub fn tab_id_for(locator: &DocumentLocator) -> makepad_widgets::LiveId {
      use waml::view::row::RowTarget;
      let target = match &locator.target {
          RowTarget::Concept(id) => format!("c:{id}"),
          RowTarget::Folder(address) => format!("f:{address}"),
          RowTarget::Virtual => "v:".to_string(),
      };
      makepad_widgets::LiveId::from_str(&format!("__doc_tab__{}__{target}", locator.surface.as_str()))
  }
  ```
- Consumes: Task 4's locator on every `OpenDocument`.

- [ ] **Step 1: Write the distinctness test** (replaces `generic_okf_identity_is_stable_and_distinct_from_uml_and_source`, `okf_documents.rs:158-167`): for one key `"order"`, the ids for `(Concept, markdown)`, `(Concept, source)`, `(Concept, canvas)` are pairwise distinct and each stable across two calls; `(Folder "/order", folder)` differs from `(Concept "order", folder-surface-whatever)`; `(Folder "/shop", source)` differs from `(Concept "shop/index", source)` — the folder-source tab and a hypothetical direct concept-open of the index are different tabs (they have different targets; document why in a comment: the folder's source tab belongs to the folder's history entry).
- [ ] **Step 2: Swap the derivation.** Providers set `tab_id: tab_id_for(&locator)` (build the locator first, then the id — restructure each `OpenDocument` literal accordingly). Delete `okf_document_tab_id`, `source_document_tab_id`, `uml_document_tab_id`, `folder_document_tab_id` and chase the compiler through their test callers (assertions comparing against e.g. `uml_document_tab_id("order")` become `tab_id_for(&DocumentLocator::concept("order", SurfaceId::canvas()))`).
- [ ] **Step 3: Gate** — watch `document_host.rs`/`doc_tabs.rs` tests that hand-build tabs with `source_document_tab_id` (`document_host.rs:740,763`): rewrite via `tab_id_for`. **Commit** — `refactor(editor): one tab-id derivation over (target, surface)`.

### Task 8: The forcing case, end to end

Spec's success criterion: "the folder case working end to end, not the refactor landing". App-level tests for the round-trip that was inexpressible before this plan, plus the negative case.

**Files:**
- Test: `crates/waml-editor/src/app/tests/navigation.rs`

**Interfaces:**
- Consumes: `open_source_for` (Task 6), the typed folder locator (Task 4), the gate (Task 3), history stopping on folder tabs (Task 2).

- [ ] **Step 1: The forcing case.** Fixture with `shop/index.md` present. Open the folder tab (`NavigationTarget::Directory { address: "/shop" }`), then `app.open_source_for(&mut cx, RowTarget::Folder("/shop".into()))`. Assert: the active tab's locator is `{ Folder("/shop"), source }`; the folder tab is still open (two tabs, or preview-replaced per the slot rules — assert whatever `transition_to_location`'s preview semantics actually produce and comment it); history Back returns to the folder tab (`active tab locator == DocumentLocator::folder("/shop")`); Forward returns to the source tab. Also: `open_source_for` twice reuses the same tab (locator hit in `tab_id_for_locator`, no tab-count growth).
- [ ] **Step 2: The root works.** Same round-trip for `RowTarget::Folder("/")` (key `"index"` — the edge the spike confirmed).
- [ ] **Step 3: The negative case.** Fixture with `loose/` linked but no `loose/index.md`: `open_source_for(Folder("/loose"))` leaves the active tab UNCHANGED (the open returns `None`, `transition_to_location` returns `false`, no blank tab, no history entry) — assert tab count and active locator before/after are identical. This is the app-level face of Task 3's gate test.
- [ ] **Step 4: Gate, commit** — `test(editor): folder source round-trips end to end; absent index offers nothing`.

---

## Deferred visual verification (post-implementation, human-run — NOT a task, NOT gate criteria)

The automated workflow cannot look at a screen. Each item ships gate-green with the check outstanding; sign-off requires a human in a real window (synthetic-click method per `interactive-verify-by-synthetic-clicks`).

- [ ] **Task 2 baseline+after:** BEFORE pulling Task 2's commit, capture the current behavior (Back jumps over a folder tab; clicking an inactive folder tab does nothing — the spike could not GUI-verify the second one and flagged a possible compensating path). AFTER: Back/Forward stops on folder tabs; clicking an inactive folder tab activates it.
- [ ] A folder tab's source view opens showing the folder's `index.md` content, with source-tab chrome (no diagram tool dock), and its tab title/icon read sensibly (FileCode + the folder's title).
- [ ] History Back from a folder's source tab lands on the folder listing, visually.
- [ ] Concept tabs (generic, class diagram, behavior, sequence, classifier) look and behave exactly as before across open/reopen/Back/Forward — the whole plan is supposed to be invisible here.
- [ ] The in-place `SourceToggleView` flip on UML tabs still works untouched (it is the next spec's demolition target, not this one's casualty).
- [ ] Projected/Raw session toggle still swaps folder-tab contents in place (`refresh_folder_tabs` now filters by target, not category).

## Self-review notes

- **Spec coverage:** §1 (locator widening + RowTarget re-keying + primary-is-a-resolution) → Tasks 1, 4; §2 (NavigationTarget surface) → Task 6; §3 (tab ids) → Task 7; §4 (or_else becomes a resolution) → Tasks 4–5 (`default_surface_for` + table routing; residual `.or_else` inside the canvas factory is documented as the stale-locator degrade, research finding 5); §5 (source per target, corrected gate) → Task 3; §6 (OpenCtx real, `resolve` dropped) → Task 1. Testing bullets: same-tab preservation → T1 S2/T5 S5; unknown-surface degrade live → T5 S4; folder Back/Forward → T2 S4; toggle/factory same tab → T6 S2; forcing case → T8 S1-2; no-index negative → T3 S1 + T8 S3; folder locator resolves/lookup → T2; hidden concept both modes → T5 S1.
- **Dead-code ledger at each boundary:** T1–T3 extend the allow'd seam (allow'd roots keep callees live); T4 deletes `DocumentKind` in the same commit that removes its last use; T5 strips the seam's allows once the locator path calls it; T7 deletes the four id fns and their callers together. No new `#[allow(dead_code)]` anywhere.
- **Degraded intermediates pushed to main:** none planned. T2 is a straight fix (flagged user-visible); T4 is shape-only; T7 changes LiveId values with no persistence. The only riskier commit is T4 by sheer width — its Step 7 forbids test-behavior changes.
- **Type consistency across tasks:** `open_locator_with_asset_host(okf, uml, locator, assets, limits, mode)` is introduced in T2 and its locator type changes in T4 with the arity stable; `SurfaceId::{markdown,source,canvas,folder}()` introduced T4, consumed T5–T8; `open_source_for_target(analysis, target, assets)` introduced T3, re-stamped T4 S2, routed T5; `tab_id_for(&DocumentLocator)` introduced T7 only.
- **Follow-on spec unblocked:** `DocTab.locator.surface` is exactly what its toggle will dispatch on; `open_source_for` is the navigation it will call; `SourceToggleView`/`GenericOkfView`/chrome untouched.
