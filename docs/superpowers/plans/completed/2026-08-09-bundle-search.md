# Bundle Search Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Bundle-wide search — a `SearchIndex` trait with a hand-rolled in-memory inverted index in the `waml` core crate, a palette popup, a results tab, a `Ctrl+F` find strip with canvas dimming, `DocView::reveal`, F3/Shift+F3 cross-document traversal, and an index asset built at static-site export.

**Architecture:** The index (trait + backend + field extraction + query language + BM25) lives entirely in `crates/waml/` behind one `SearchIndex` trait; surfaces consume `Vec<Hit>` and never see the index. The editor adds one `SearchState` seam (build on bundle open, update per document on save), one new popup surface (`PalettePopup` over the existing `PopupRoot`), one new body surface + `DocView` (`SearchResultsView`, a tab like any other), one overlay strip (`FindStrip`), and one new trait method (`DocView::reveal(cx, body, target)`) that the results tab, find strip, and F3 traversal all call.

**Tech Stack:** Rust (makepad widgets), no new dependencies anywhere. Spec: `docs/superpowers/specs/2026-08-09-bundle-search-design.md`.

## Global Constraints

- **Gate (every task must leave it green):** `cargo test --workspace` AND `pnpm -C editors/vscode test && pnpm -C editors/vscode lint && pnpm -C editors/vscode build`. Clippy runs with `-D warnings`: unused `pub(crate)`/private items are hard errors, so never land scaffolding nothing uses. `waml`, `waml-editor`, and `waml-markdown-editor` are lib crates — genuinely `pub` seam items are not dead code, and trait-impl methods are never dead code.
- **`crates/waml/` (core): NO new dependencies.** Existing deps only (`regex`, `miniz_oxide`, `ttf-parser`, `waml-syntax`; the optional `serde` feature stays optional and unused here).
- **Core must stay wasm-clean** for `wasm32-unknown-unknown` under the makepad runtime (NOT wasm-bindgen): no threads, no filesystem, no `getrandom`, and **absolutely no `SystemTime::now()`** (panics on makepad wasm). Verify with `cargo check -p waml --target wasm32-unknown-unknown` in every core task.
- **`SearchIndex` is the only thing surfaces depend on.** Editor/CLI code imports `waml::search::{SearchIndex, Hit, Snippet, QueryScope, FieldGroup, HitTarget}` — never `MemSearchIndex` internals — so a native-only tantivy backend can be swapped in later without touching any surface.
- **Out of scope — do NOT implement:** phrase/fuzzy/regex/boolean queries, cross-bundle search, persisted on-disk index, split view / multi-pane hosting, preview panes, `?q=` deep links, tantivy.
- **No visual verification by the implementer.** Anything that needs eyes on a running GUI goes to the "Visual sign-off owed" section at the end — it never blocks a task.
- Commit messages follow the `feat:`/`docs:`/`test:` conventions seen in `git log`.

## File Structure

New files (each created by its named task):

- `crates/waml/src/search/mod.rs` — public search API: `FieldGroup`, `Hit`, `HitTarget`, `Snippet`, `QueryScope`, `SearchIndex` trait
- `crates/waml/src/search/tokenize.rs` — tokenizer
- `crates/waml/src/search/query.rs` — query parser (terms, prefix, `kind:` / `in:` filters)
- `crates/waml/src/search/extract.rs` — per-document field extraction (names/prose/model/structure) + span dedupe
- `crates/waml/src/search/index.rs` — `MemSearchIndex`: inverted index, BM25, snippets
- `crates/waml/src/search/asset.rs` — versioned serialize/deserialize for the export-site index asset
- `crates/waml/tests/search_golden.rs` — golden query suite over `docs/waml/`
- `crates/waml-editor/src/search_state.rs` — editor-side index lifecycle (build on open, update on save) + hidden-by-projection set
- `crates/waml-editor/src/search_session.rs` — live session: ordered hits, cursor, F3 traversal order
- `crates/waml-editor/src/search_results_view.rs` — `SearchResultsView` (`DocView`) + grouping model
- `crates/waml-editor/src/popup/palette.rs` — `PalettePopup` widget + palette section model
- `crates/waml-editor/src/find_strip.rs` — `FindStrip` widget + counter model

Key modified files: `crates/waml/src/lib.rs`, `crates/waml-editor/src/doc_view.rs` (`RevealTarget`, `DocView::reveal`), `crates/waml-editor/src/app/event.rs` (shortcuts), `crates/waml-editor/src/shortcuts.rs`, `crates/waml-editor/src/popup/root.rs` (+`PopupSpec::Palette`), `crates/waml-editor/src/app.rs` (live design: body surface, find strip overlay), `crates/waml-editor/src/documents.rs` (search surface open path), `crates/waml-markdown-editor/src/presentation/draw.rs` (`DecorationRole::SearchMatch`), `crates/waml-editor/src/canvas/class/widget.rs` (spotlight), `crates/waml-cli/src/site.rs` + `crates/waml-cli/src/commands.rs` (index asset at export), `crates/waml-editor/src/browser_boot.rs` (load index asset), `crates/waml-ui-test/*` (DSL ops), `docs/waml/goals/mvp.md`, `docs/waml/goals/beyond-uml.md`.

## Decisions the spec left open (made here, used consistently below)

1. **Palette hotkey:** the spec never names one. This plan uses **Ctrl+K** (Cmd+K on macOS) — unclaimed by the editor (`shortcuts.rs` claims Ctrl+Z/Y; unmodified V/N/C/?/T; Escape) and unclaimed by makepad.
2. **Empty-query palette recents:** the spec points at "the recents the start screen already tracks", but those are recently opened *models* (`config::recents()`, also empty on wasm), not documents inside a bundle. This plan shows the most recent distinct documents from the in-session `ViewHistory` instead — the same idea at document granularity, identical on the exported site.
3. **Prefix matching:** every query term matches as a prefix; an exact token match gets a score boost. (The spec only requires that "paym" match "payment".)
4. **Cross-group ranking:** strict tiers — sort by (`FieldGroup` tier, then BM25 desc). This satisfies "name hits always outrank body hits" by construction.
5. **`kind:` filter** keeps hits whose element/document UML kind matches; **`in:`** is a document-path prefix filter. Unknown filter names degrade to plain terms (spec rule).
6. **Index-building degraded state:** v1 builds synchronously (single-digit ms at current bundle size). The `TextIndexStatus { Building, Ready }` enum and the palette rendering of `indexing…` still land and are unit-tested by constructing the `Building` state directly; no async build machinery in v1.
7. **Search tab identity:** `DocumentLocator { target: RowTarget::Virtual, surface: SurfaceId(format!("search:{query}")) }`. `tab_id_for` (`crates/waml-editor/src/documents.rs`) already bakes the surface string into the tab id, so two queries are two tabs and one query re-activates its tab. No new `RowTarget` variant (that would ripple through every tree consumer).
8. **Activating a projection-hidden hit** opens a small confirm `MenuPopup` ("Show hidden match" / dismiss); confirming opens the document and reveals the span. (Spec: "Activating one offers to reveal the masked content.")
9. **Hidden-by-projection detection:** a hit is hidden iff its concept is absent from the projected tree (`tree.rs` build output) while present in the bundle. Document-level hiding is what the folder-view middleware chains actually do today; element-level masking is not modelled in v1.
10. **Index asset:** the index file name is **derived from the bundle file name by appending `.search-index`** — `bundle.waml` → `bundle.waml.search-index`, `orders.waml` → `orders.waml.search-index`. It is not a fixed name. `BUNDLE_FILE` (`crates/waml-cli/src/site.rs`) is only the default the CLI writes; boot resolves whatever `?bundle=<name>` supplies into `BrowserBootSource::Bundle(String)`, and two bundles can sit in one served directory. A fixed name would bind the index to the wrong bundle, and because the hash check would then fail into a local rebuild, the symptom would be "the published site is slow" rather than an error. Appending rather than replacing the extension keeps the derivation total for any bundle name and cannot collide with a real bundle. The asset carries a format version and an FNV-1a content hash of the bundle; the editor falls back to building locally when the asset is missing, version-mismatched, or hash-stale — so native and exported site share one code path with one optional fast-load.

---

### Task 1: Core query model — tokenizer and query parser

**Files:**
- Create: `crates/waml/src/search/mod.rs`, `crates/waml/src/search/tokenize.rs`, `crates/waml/src/search/query.rs`
- Modify: `crates/waml/src/lib.rs` (add `pub mod search;` to the alphabetical module list)

**Interfaces:**
- Consumes: nothing (leaf module).
- Produces: `waml::search::tokenize::{tokenize, Token}` with `Token { text: String, start: u32, end: u32 }` (byte offsets, text lowercased); `waml::search::query::{parse_query, ParsedQuery, QueryTerm, QueryFilter}`. Task 3 consumes `ParsedQuery`; Tasks 10/12 echo `ParsedQuery::filters` back in empty-state UI.

- [ ] **Step 1: Write the failing tests** (`#[cfg(test)] mod tests` in each file):

```rust
// tokenize.rs
#[test]
fn words_lowercase_with_byte_offsets() {
    let tokens = tokenize("Payment is Captured");
    assert_eq!(
        tokens.iter().map(|t| (t.text.as_str(), t.start, t.end)).collect::<Vec<_>>(),
        vec![("payment", 0, 7), ("is", 8, 10), ("captured", 11, 19)]
    );
}
#[test]
fn punctuation_splits_and_identifiers_survive() {
    let texts: Vec<String> =
        tokenize("id: payment-capture (v2)").into_iter().map(|t| t.text).collect();
    assert_eq!(texts, vec!["id", "payment", "capture", "v2"]);
}
#[test]
fn non_ascii_words_are_kept_whole() {
    let texts: Vec<String> = tokenize("Zahlung erfasst").into_iter().map(|t| t.text).collect();
    assert_eq!(texts, vec!["zahlung", "erfasst"]);
}

// query.rs
#[test]
fn bare_terms_are_anded_terms() {
    let q = parse_query("payment capture");
    assert_eq!(q.terms.len(), 2);
    assert!(q.filters.is_empty());
}
#[test]
fn known_filters_parse_and_unknown_filters_degrade_to_terms() {
    let q = parse_query("kind:actor in:docs/guides/ payment sev:high");
    assert_eq!(q.filters, vec![
        QueryFilter::Kind("actor".into()),
        QueryFilter::InPath("docs/guides/".into()),
    ]);
    let texts: Vec<&str> = q.terms.iter().map(|t| t.text.as_str()).collect();
    assert_eq!(texts, vec!["payment", "sev", "high"]);
}
#[test]
fn empty_and_whitespace_queries_are_empty() {
    assert!(parse_query("").is_empty());
    assert!(parse_query("   ").is_empty());
}
```

- [ ] **Step 2: Run to verify failure.** Run: `cargo test -p waml search::` — expect a compile error (module missing).

- [ ] **Step 3: Implement.** `tokenize`: iterate `char_indices`, accumulate runs of `c.is_alphanumeric() || c == '_'`, lowercase, record byte start/end. `query.rs`:

```rust
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum QueryFilter {
    /// `kind:actor` — restrict to elements/documents of this UML kind.
    Kind(String),
    /// `in:docs/guides/` — restrict to documents whose path starts with this.
    InPath(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QueryTerm { pub text: String } // lowercased

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ParsedQuery {
    pub terms: Vec<QueryTerm>,
    pub filters: Vec<QueryFilter>,
}

impl ParsedQuery {
    pub fn is_empty(&self) -> bool { self.terms.is_empty() && self.filters.is_empty() }
}

/// Whitespace-split first; a chunk `name:value` with a KNOWN name (`kind`,
/// `in`) is a filter; every other chunk falls through the tokenizer as plain
/// terms (spec: unknown filter names are terms, never errors).
pub fn parse_query(input: &str) -> ParsedQuery { /* … */ }
```

`mod.rs` declares `pub mod query; pub mod tokenize;` for now.

- [ ] **Step 4: Run tests + wasm check.** `cargo test -p waml search::` then `cargo check -p waml --target wasm32-unknown-unknown` — PASS / clean. (`rustup target add wasm32-unknown-unknown` if missing.)

- [ ] **Step 5: Full gate, commit**

```bash
cargo test --workspace
git add crates/waml/src/lib.rs crates/waml/src/search
git commit -m "feat(search): query tokenizer and filter parser in core"
```

---

### Task 2: Core field extraction — four field groups with span dedupe

**Files:**
- Create: `crates/waml/src/search/extract.rs`
- Modify: `crates/waml/src/search/mod.rs` (add `pub mod extract;` plus the shared hit vocabulary below)

**Interfaces:**
- Consumes: `waml::source::SourceBundle`, `waml::analysis::OkfAnalysis` (bundle concepts/directories, catalog, frontmatter), `waml::uml::Analysis` (projection: classifiers, kinds, relationships), `waml-syntax` markdown parse (headings, links, text runs).
- Produces in `mod.rs` (the crate-public search vocabulary — later tasks use these exact names):

```rust
/// Ranking tier order is declaration order: Names > Model > Prose > Structure.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum FieldGroup { Names, Model, Prose, Structure }

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum HitTarget {
    /// Byte span in the document raw source, with a 1-based line for display.
    TextSpan { start: u32, end: u32, line: u32 },
    /// A model element reference (concept id / classifier key).
    ModelElement { key: String },
}

#[derive(Clone, Debug, PartialEq)]
pub struct Hit {
    pub document: String,            // bundle-relative path, e.g. "guides/checkout.md"
    pub concept_id: Option<String>,
    pub group: FieldGroup,
    pub target: HitTarget,
    pub score: f32,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct QueryScope { pub document: Option<String> }

#[derive(Clone, Debug, PartialEq)]
pub struct Snippet { pub text: String, pub highlights: Vec<(usize, usize)> }
```

- Produces in `extract.rs`:

```rust
#[derive(Clone, Debug, PartialEq)]
pub struct FieldEntry {
    pub group: FieldGroup,
    pub text: String,      // raw text to index; the tokenizer runs over this
    pub target: HitTarget, // where a match on this entry lands
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct DocumentFields {
    pub path: String,
    pub concept_id: Option<String>,
    pub title: String,
    /// UML kind of the primary element, lowercased ("class", "actor", …),
    /// for the `kind:` filter. None for plain markdown.
    pub kind: Option<String>,
    pub entries: Vec<FieldEntry>,
}

pub fn extract_bundle(
    source: &crate::source::SourceBundle,
    okf: &crate::analysis::OkfAnalysis,
    uml: &crate::uml::Analysis,
) -> Vec<DocumentFields>;
```

- [ ] **Step 1: Write failing tests** in `extract.rs` over a small inline bundle. Build it with `SourceBundle::try_from_pairs` (see `crates/waml-editor/src/bundle_export.rs` tests for the shape) and run the analysis pipeline the way `crates/waml/tests/incremental_analysis.rs` constructs `OkfAnalysis` + `uml::Analysis` — copy its helper. Tests:

```rust
#[test]
fn a_class_document_contributes_all_four_groups() {
    // names: class name + document title; model: "class" kind + relationship
    // endpoint names; prose: a body sentence; structure: frontmatter keys +
    // a markdown link target.
}
#[test]
fn names_entries_target_the_model_element_and_prose_entries_target_text_spans() {}
#[test]
fn overlapping_raw_and_projected_prose_dedupe_to_one_entry_per_span() {
    // The same sentence must not appear twice with the same (start, end).
}
#[test]
fn a_plain_markdown_document_contributes_names_prose_structure_but_no_model() {}
```

- [ ] **Step 2: Run to verify failure.** `cargo test -p waml search::extract` — FAIL.

- [ ] **Step 3: Implement.** Per document: title + headings → `Names` (heading target = its `TextSpan`); UML classifier name (when `uml` claims the concept) → `Names` with `HitTarget::ModelElement`; kind, relationship endpoint pairs, tags (`uml.projection`) → `Model` with `ModelElement` targets; markdown body text runs (parse via `waml-syntax`, skip markup and code-fence markers, record byte spans and 1-based lines) → `Prose`; frontmatter keys (`crate::frontmatter`), `id:` values, link targets → `Structure`. Dedupe prose spans with a `HashSet<(u32, u32)>` per document.

- [ ] **Step 4: Run + wasm check.** `cargo test -p waml search::` and `cargo check -p waml --target wasm32-unknown-unknown` — PASS / clean.

- [ ] **Step 5: Gate + commit**

```bash
cargo test --workspace
git add crates/waml/src/search
git commit -m "feat(search): four-field-group document extraction"
```

---

### Task 3: `SearchIndex` trait and the in-memory inverted index

**Files:**
- Create: `crates/waml/src/search/index.rs`
- Modify: `crates/waml/src/search/mod.rs` (add `pub mod index;`, the trait, re-export `MemSearchIndex`)

**Interfaces:**
- Consumes: `ParsedQuery` (Task 1), `DocumentFields`/`FieldEntry` (Task 2).
- Produces in `mod.rs`:

```rust
/// The engine boundary (spec §Engine boundary). Surfaces depend on THIS,
/// never on a backend, so a native-only tantivy backend can slot in later.
pub trait SearchIndex {
    fn update_document(&mut self, path: &str, fields: extract::DocumentFields);
    fn remove_document(&mut self, path: &str);
    fn query(&self, query: &str, scope: &QueryScope) -> Vec<Hit>;
    fn snippet(&self, hit: &Hit, width: usize) -> Snippet;
}
```

- Produces in `index.rs`:

```rust
pub struct MemSearchIndex { /* postings, docs, kinds, entry texts for snippets */ }

impl MemSearchIndex {
    /// build(documents) -> Index (spec). Also the empty-index constructor.
    pub fn build(documents: impl IntoIterator<Item = extract::DocumentFields>) -> Self;
}
impl SearchIndex for MemSearchIndex { /* … */ }
```

- Ranking contract (pin with tests): sort by `FieldGroup` tier first, BM25 (k1 = 1.2, b = 0.75) descending within a tier; all terms ANDed; every term matches as a token prefix; an exact token match multiplies that term contribution by 2.0; final ties break on `(document, target)` for determinism. `kind:`/`in:` filters drop non-matching hits before ranking. `Hit.score` carries the BM25 value; tier ordering is applied in the final sort, not encoded in the float.

- [ ] **Step 1: Write failing tests** (`#[cfg(test)]` in `index.rs`) against a hand-built 3-document `Vec<DocumentFields>` fixture (literal structs, no analysis needed):

```rust
#[test] fn a_name_hit_outranks_a_prose_hit_for_the_same_term() {}
#[test] fn prefix_matches_as_you_type() { /* "paym" finds "payment" entries */ }
#[test] fn terms_are_anded() { /* "payment capture" hits only docs with both */ }
#[test] fn kind_filter_restricts_and_in_filter_scopes_by_path_prefix() {}
#[test] fn scope_restricts_to_one_document() {}
#[test] fn update_document_replaces_and_remove_document_forgets() {}
#[test] fn snippet_windows_the_span_on_char_boundaries_with_highlight_ranges() {}
#[test] fn empty_queries_return_empty_not_panic() {}
```

- [ ] **Step 2: Run to verify failure.** `cargo test -p waml search::index` — FAIL.

- [ ] **Step 3: Implement.** `HashMap<String /*token*/, Vec<Posting { doc: u32, entry: u32, tf: u32 }>>` plus a sorted `Vec<String>` token list for prefix range scans (`partition_point` over the prefix). IDF from document frequency; per-group average entry length for the BM25 length norm. Deterministic iteration only (sort before emit) — no clocks, no randomness, no hashing that leaks into ordering.

- [ ] **Step 4: Run + wasm check.** `cargo test -p waml search::` and `cargo check -p waml --target wasm32-unknown-unknown` — PASS / clean.

- [ ] **Step 5: Gate + commit**

```bash
cargo test --workspace
git add crates/waml/src/search
git commit -m "feat(search): SearchIndex trait and in-memory BM25 inverted index"
```

---

### Task 4: Golden query suite over the `docs/waml/` bundle

Ranking has no compile error when it is wrong; this suite is the defence, and it lands NOW — before any surface — so every later core change diffs against it.

**Files:**
- Create: `crates/waml/tests/search_golden.rs`
- Create: `crates/waml/tests/fixtures/search_golden.txt` (the expected-results golden file)

**Interfaces:**
- Consumes: `waml::search::{MemSearchIndex, SearchIndex, QueryScope}`, `extract::extract_bundle`, plus the repo-root discovery pattern from `crates/waml/tests/sequence_no_legacy.rs` (which already resolves `docs/waml` relative to the workspace root).

- [ ] **Step 1: Write the harness.** Load every `**/*.md` under `docs/waml/` (skip `.waml/` settings directories) into a `SourceBundle` via `try_from_pairs` with bundle-relative paths; run the same analysis constructors Task 2 used; `extract_bundle` + `MemSearchIndex::build`. For a fixed query list, format results as stable text — one line per hit: `query | rank | group | document | target-kind` — capped at the top 8 hits per query. Query set (verbatim):

```text
payment
mvp
goals
bundle
kind:actor
projection
in:goals/ scope
sequence lifeline
markdown editor
search
```

If a query legitimately returns nothing against `docs/waml`, the golden file records `query | (no results)` — an empty answer is a pinned answer too. Adjust the list during implementation ONLY by swapping in terms that do appear in `docs/waml` (keep 10 queries: at least one `kind:` filter, one `in:` filter, one multi-term, one prefix-ish short term).

- [ ] **Step 2: Generate the golden file.** Run the test with an env flag (`WAML_BLESS=1` writes `fixtures/search_golden.txt`, mirroring how existing golden tests in `crates/waml/tests/golden.rs` manage blessing — reuse its mechanism if present, otherwise implement the flag). Inspect the output by hand for sanity: `Payment`-style name hits above prose, filters filtering.

- [ ] **Step 3: Run to verify green without the flag.** `cargo test -p waml --test search_golden` — PASS, and re-running is deterministic.

- [ ] **Step 4: Gate + commit**

```bash
cargo test --workspace
git add crates/waml/tests/search_golden.rs crates/waml/tests/fixtures/search_golden.txt
git commit -m "test(search): golden query suite over the docs/waml bundle"
```

---

### Task 5: Editor `SearchState` — build on bundle open, update on save

**Files:**
- Create: `crates/waml-editor/src/search_state.rs`
- Modify: `crates/waml-editor/src/lib.rs` (register the module), `crates/waml-editor/src/app.rs` (one `search: SearchState` field on `App`), `crates/waml-editor/src/app/workspace.rs` (rebuild where a bundle/session is installed), `crates/waml-editor/src/app/event.rs` (`handle_persistence_event`: refresh after a successful save flush)

**Interfaces:**
- Consumes: `waml::search::{MemSearchIndex, SearchIndex, Hit, QueryScope}`, `extract_bundle`, `EditorSession` snapshot (source + analyses).
- Produces (all `pub` — this is the lib-crate seam every surface task uses):

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextIndexStatus { Building, Ready }

pub struct SearchState {
    index: waml::search::MemSearchIndex, // the ONLY place the backend type appears
    status: TextIndexStatus,
}

impl SearchState {
    pub fn empty() -> Self;
    /// Full rebuild from the live session (bundle open / session replace).
    pub fn rebuild(&mut self, source: &SourceBundle, okf: &OkfAnalysis, uml: &waml::uml::Analysis);
    /// Per-document refresh (document save), spec §Index lifecycle.
    pub fn refresh_document(&mut self, path: &str, source: &SourceBundle, okf: &OkfAnalysis, uml: &waml::uml::Analysis);
    pub fn status(&self) -> TextIndexStatus;
    pub fn query(&self, query: &str, scope: &waml::search::QueryScope) -> Vec<waml::search::Hit>;
    pub fn snippet(&self, hit: &waml::search::Hit, width: usize) -> waml::search::Snippet;
}
```

- Wiring (all in this task, so nothing is dead): `App` gains the field; the function in `app/workspace.rs` that installs a fresh `EditorSession` (follow the call path from `open_model_via_picker` / the start-screen open to where analyses first exist) calls `rebuild`; `handle_persistence_event` (`app/event.rs`) calls `refresh_document` for the flushed documents after a successful `save_or_retry`. v1 sets `status = Ready` at the end of a synchronous `rebuild` — `Building` exists for the palette state and for a later async backend.

- [ ] **Step 1: Write failing tests.** Unit tests in `search_state.rs` (build a session-shaped fixture exactly as Task 2 tests did): after `rebuild`, a query for a known class name returns a `Names` hit; after editing a document pair and `refresh_document`, the old term is gone and the new term hits; `status()` is `Ready` after rebuild. Plus one app-level test in `crates/waml-editor/src/app/tests/workspace.rs` following that file's existing fixture pattern: opening the test workspace leaves `app.search.query("…") ` non-empty for a term from the fixture bundle.

- [ ] **Step 2: Run to verify failure.** `cargo test -p waml-editor search_state` — FAIL.

- [ ] **Step 3: Implement + wire** as described above.

- [ ] **Step 4: Run.** `cargo test -p waml-editor` — PASS.

- [ ] **Step 5: Gate + commit**

```bash
cargo test --workspace
git add crates/waml-editor/src
git commit -m "feat(editor): SearchState builds the index on open and refreshes on save"
```

---

### Task 6: `RevealTarget`, `DocView::reveal`, and the text-surface implementation

**Files:**
- Modify: `crates/waml-editor/src/doc_view.rs` (`RevealTarget` + trait method with default no-op)
- Modify: `crates/waml-markdown-editor/src/presentation/draw.rs` (`DecorationRole::SearchMatch`) and the presentation pipeline that emits `DrawCommand::Decoration`
- Modify: `crates/waml-markdown-editor/src/widget.rs` + `src/reading.rs` (public API: set/clear search highlight ranges, reveal a range)
- Modify: `crates/waml-editor/src/source_view.rs`, `crates/waml-editor/src/markdown_hosts.rs` views, `crates/waml-editor/src/source_toggle_view.rs` (delegate), `crates/waml-editor/src/folder_view.rs` + `generic_okf_view.rs` (keep default)

**Interfaces:**
- Produces (in `doc_view.rs` — the ONE reveal path the results tab, find strip, and F3 all call):

```rust
/// What a search hit asks the active view to show (spec §DocView::reveal).
#[derive(Clone, Debug, PartialEq)]
pub enum RevealTarget {
    TextSpan { start: u32, end: u32 },
    ModelElement { key: String },
}

// on trait DocView (default: no-op so folder/generic views compile unchanged):
fn reveal(&mut self, cx: &mut Cx, body: &BodyWidgets, target: &RevealTarget) {
    let _ = (cx, body, target);
}
```

- Produces (markdown editor crate, `pub` on `MarkdownEditor`/`MarkdownViewer` and their `Ref` ext traits): `set_search_highlights(cx, ranges: Vec<TextRange>)`, `clear_search_highlights(cx)`, `reveal_range(cx, range: TextRange)` (scroll the range into view, reusing the existing caret/fragment scroll machinery in `input::ScrollState`).
- Consumes: `DecorationRole` / `DrawCommand::Decoration` layering (already exists — `SearchMatch` is one more role rendered like `LinkUnderline` but as a fill-behind, at the decoration layer).

- [ ] **Step 1: Failing tests, markdown side.** Extend `crates/waml-markdown-editor/tests/draw_layers.rs` (and `reading_widget_draw.rs` for the viewer): after `set_search_highlights` with one range, the draw command list contains a `Decoration { role: DecorationRole::SearchMatch, .. }` covering that range; after `clear_search_highlights`, it does not. Follow the existing test style in those files exactly.

- [ ] **Step 2: Failing tests, editor side.** In `source_view.rs` tests (and the markdown host view tests): `reveal(TextSpan)` on a `SourceView` forwards to `set_search_highlights` + `reveal_range` (assert via the draw-command list and the scroll state, the same handles the widget-parity tests use). `SourceToggleView::reveal` delegates to whichever surface is showing.

- [ ] **Step 3: Run to verify failure**, then implement: the new role + its draw color (reuse the theme's selection/link color roles — pick the existing highlight-ish `ColorRole`, do NOT invent a new theming channel), the widget APIs, the `DocView` impls. `SourceView`/markdown-host views map `RevealTarget::TextSpan` onto their surface; they ignore `ModelElement`.

- [ ] **Step 4: Run.** `cargo test -p waml-markdown-editor -p waml-editor` — PASS.

- [ ] **Step 5: Gate + commit**

```bash
cargo test --workspace
git add crates/waml-markdown-editor crates/waml-editor/src
git commit -m "feat(editor): DocView::reveal with markdown search-match decoration"
```

---

### Task 7: Canvas reveal and the search spotlight state

**Files:**
- Modify: `crates/waml-editor/src/canvas/class/widget.rs` (`ClassDiagramSurface`: spotlight state + `set_search_spotlight`)
- Modify: `crates/waml-editor/src/class_diagram_view.rs`, `crates/waml-editor/src/classifier_preview_view.rs`, `crates/waml-editor/src/behavior_doc_view.rs` (`reveal` impls)

**Interfaces:**
- Produces (`pub` on `ClassDiagramSurface`): `set_search_spotlight(&mut self, cx: &mut Cx, lit: Option<HashSet<String>>)` — `Some(keys)`: nodes in `keys` render normally, everything else dims; `None`: clear. Also `search_spotlight(&self) -> Option<&HashSet<String>>` for tests. Dimming is a per-node draw modulation composed WITH (not replacing) selection/hover/conflict state — implement as an alpha/desaturation factor applied at node draw time, exactly where hover state already modulates.
- Produces: `ClassDiagramView::reveal(ModelElement { key })` selects the node, centres the camera on it, and repoints the inspector — reuse the existing `restore_anchor` machinery (`ViewAnchor::Diagram { selected_key, camera }`) rather than a second camera path. `ClassifierPreviewView::reveal` re-focuses via the `build_focus_scene` + `set_focus` path it already uses. `behavior_doc_view` maps `ModelElement` to its own selection affordance; `TextSpan` is ignored by canvas views.

- [ ] **Step 1: Failing tests.** In `canvas/class/widget.rs` (or `scene.rs` where node draw state is computed — follow where `set_focus` tests live): setting a spotlight marks non-listed nodes dimmed in the computed draw state and listed nodes not-dimmed; clearing restores. In `class_diagram_view.rs` tests: `reveal(ModelElement)` leaves the view with `capture_anchor` reporting `selected_key == Some(key)` and a camera centred on that node rect.

- [ ] **Step 2: Run to verify failure**, then implement.

- [ ] **Step 3: Run.** `cargo test -p waml-editor` — PASS.

- [ ] **Step 4: Gate + commit**

```bash
cargo test --workspace
git add crates/waml-editor/src
git commit -m "feat(canvas): search spotlight dimming and model-element reveal"
```

---

### Task 8: Results view — file creation (grouping model + `SearchResultsView` + body surface)

**Files:**
- Create: `crates/waml-editor/src/search_results_view.rs`
- Modify: `crates/waml-editor/src/lib.rs` (module), `crates/waml-editor/src/app.rs` (live design: add a `search_results_surface` sibling of `folder_view_surface` holding a scrolling row list — clone the folder-view surface structure), `crates/waml-editor/src/doc_view.rs` (`BodyWidgets`: `search_results(&self) -> WidgetRef` accessor + `show_search_results(cx)` that hides the other five surfaces, mirroring `show_folder_view`)

**Interfaces:**
- Consumes: `waml::search::{Hit, Snippet, FieldGroup, HitTarget}`, `SearchState` (Task 5), `DocView`/`BodyChrome` (existing), `RevealTarget` (Task 6).
- Produces (`pub`, consumed by Task 9/11/14/16):

```rust
pub enum RowLabel { Name, Rel, Doc, Id, Link, Line(u32) }

pub struct ResultRow {
    pub hit: waml::search::Hit,
    pub label: RowLabel,
    pub snippet: waml::search::Snippet,
    pub hidden: bool, // hidden by the active projection -> muted + badge
}

pub struct DocumentGroup {
    pub path: String,        // "billing.waml"
    pub directory: String,   // "docs/waml/domain/"
    pub collapsed: bool,
    pub rows: Vec<ResultRow>,
}

/// Grouping: rows grouped by document, groups ordered by their best hit,
/// rows within a group in rank order (spec §Results tab).
pub fn group_hits(rows: Vec<ResultRow>) -> Vec<DocumentGroup>;

pub struct SearchResultsView {
    pub query: String,
    groups: Vec<DocumentGroup>,
    /// F3 traversal marks the current row here (Task 14).
    cursor: Option<(usize, usize)>,
}

impl SearchResultsView {
    pub fn new(query: String, rows: Vec<ResultRow>) -> Self;
    pub fn counts(&self) -> (usize /*hits*/, usize /*documents*/, usize /*hidden*/);
}
impl DocView for SearchResultsView { /* identity: add DocViewIdentity::SearchResults;
    sync pushes groups into the body list; chrome: BodyChrome::HIDDEN-like with
    breadcrumb off; handle: row clicks -> ViewOutcome.navigation (Task 9);
    header collapse toggles flip DocumentGroup.collapsed */ }
```

- The header line renders `🔍 {query}    {hits} in {documents} documents` (plus `, {hidden} hidden` when non-zero — spec §States, hidden-only results). Collapsed groups render header-only. Add `DocViewIdentity::SearchResults` to the enum (the enum already carries `#[allow(dead_code)]` and consumers match non-exhaustively, so the new variant compiles everywhere).
- **Gate-green justification for a creation task:** the view type, grouping fn, and `BodyWidgets` accessors are `pub` lib items exercised by unit tests here; the live-design surface is registered (registration is usage). Cross-file app wiring lands in Task 9.

- [ ] **Step 1: Failing tests** in `search_results_view.rs`: `group_hits` groups by document, orders groups by best rank, keeps rank order within a group; `counts()` reports hits/documents/hidden; label mapping (`Names`+`ModelElement` → `Name`, `Model` → `Rel`, `Prose` → `Line(n)`, `Structure` id vs link target → `Id`/`Link`); a collapsed group still counts.

- [ ] **Step 2: Run to verify failure**, then implement, including the live-design surface block and the two `BodyWidgets` methods (update `show_canvas`/`show_markdown_editor`/`show_markdown_viewer`/`show_folder_view` to also hide `search_results_surface` — grep each `set_visible` cluster in `doc_view.rs`).

- [ ] **Step 3: Run.** `cargo test -p waml-editor` — PASS.

- [ ] **Step 4: Gate + commit**

```bash
cargo test --workspace
git add crates/waml-editor/src
git commit -m "feat(editor): SearchResultsView with grouped, collapsible result model"
```

---

### Task 9: Results tab wiring — open path, activation, hidden hits, `SearchSession`

**Files:**
- Create: `crates/waml-editor/src/search_session.rs`
- Modify: `crates/waml-editor/src/search_state.rs` (hidden-set helper), `crates/waml-editor/src/documents.rs` (search surface arm in `locator_opens` + the open factory), `crates/waml-editor/src/app.rs` / `app/actions.rs` (open-results command + row activation + pending reveal), `crates/waml-editor/src/app/navigation.rs` (reveal-after-open)

**Interfaces:**
- Produces (`crates/waml-editor/src/search_session.rs`, `pub`):

```rust
/// The live query (spec §Search session): ordered hits in results-tab order,
/// a cursor, and the scope. Owned by App; results tab, find strip and F3 all
/// walk THIS list — there is no second ordering.
pub struct SearchSession {
    pub query: String,
    pub hits: Vec<waml::search::Hit>, // flattened group_hits order
    pub cursor: Option<usize>,
    pub scope: waml::search::QueryScope,
}
impl SearchSession {
    pub fn advance(&mut self, forward: bool) -> Option<&waml::search::Hit>; // wraps
}
```

- Produces (`SearchState`): `pub fn hidden_documents(&okf, &uml) -> HashSet<String>` — concepts present in the bundle but absent from the projected tree (build the tree the way `tree_panel.rs` does and diff; decision 9).
- Produces (App): `pub(crate) fn open_search_results(&mut self, cx, query: &str)` — runs the query, builds `ResultRow`s (snippets width 80, `hidden` from `hidden_documents`), opens a tab via `DocumentCommand::Open` with `OpenDocument { tab_id: tab_id_for(&locator), locator, title: format!("Search: {query}"), presentation: search presentation (Icon: pick an existing magnifier-ish `Icon`; category `NavCategory::Note`), view: Box::new(SearchResultsView::new(..)) }` where `locator = DocumentLocator::new(RowTarget::Virtual, SurfaceId(format!("search:{query}")))` (decision 7). Same query → same tab id → re-activates.
- Row activation: `SearchResultsView::handle` returns `ViewOutcome { navigation: Some(NavigationIntent::Resolved { target: Document { concept_id, surface: model-hit ? canvas : source/markdown, fragment: None }, disposition: Preview }), .. }` plus a NEW `ViewOutcome` field `pub reveal: Option<(String /*concept*/, RevealTarget)>` that the shell stores as `pending_reveal` and applies in `handle_draw_restores` (`app/event.rs`) after the target tab has drawn — the same deferred pattern `apply_pending_fragment` uses. Model hits carry `RevealTarget::ModelElement`, prose/structure hits `RevealTarget::TextSpan` (spec §Activation per document kind).
- Hidden rows: activating one first pops a confirm `MenuPopup` via the existing `PopupRequest::NodeContextMenu`-style route — add a `PopupRequest::Confirm { anchor: DVec2, title: String, tag: LiveId }` variant handled by the shell as a one-item `MenuPopup`; on commit, proceed with the normal open+reveal (decision 8).
- History: `locator_opens` (`documents.rs`) returns `true` for `search:`-prefixed surfaces, and the locator-driven reopen path rebuilds the view by re-running `open_search_results` with the query parsed off the surface string — so tab history traversal works (spec: participates in history like any other tab).

- [ ] **Step 1: Failing tests.** `search_session.rs`: `advance` walks forward/backward and wraps. `documents.rs` tests: `tab_id_for` gives distinct stable ids for two queries; `locator_opens` accepts a search locator. App test (in `app/tests/workspace.rs` or `navigation.rs`, existing fixture style): `open_search_results` opens a tab titled `Search: <q>`; activating the same query again activates rather than duplicates; activating a row yields a navigation to the hit document and a pending reveal.

- [ ] **Step 2: Run to verify failure**, then implement.

- [ ] **Step 3: Run.** `cargo test -p waml-editor` — PASS.

- [ ] **Step 4: Gate + commit**

```bash
cargo test --workspace
git add crates/waml-editor/src
git commit -m "feat(editor): search results tab opens, activates and reveals hits"
```

---

### Task 10: Palette popup — file creation (widget + section model + `PopupRoot` route)

**Files:**
- Create: `crates/waml-editor/src/popup/palette.rs`
- Modify: `crates/waml-editor/src/popup/mod.rs` (module), `crates/waml-editor/src/popup/root.rs` (`PopupSpec::Palette`, `ActiveKind::Palette`, route arm, live-design child)

**Interfaces:**
- Consumes: `waml::search::{Hit, FieldGroup, HitTarget, Snippet}`, `TextIndexStatus` (Task 5), the `Popup` trait + `PopupVerdict`/`PopupResult` in `popup/base.rs`, `MenuPopup` as the structural precedent (`popup/menu.rs`).
- Produces (`pub`, the pure model is what Tasks 11/16 test against):

```rust
pub enum PaletteRowKind {
    Concept { kind: String },                   // "class Payment    domain/billing.waml"
    Document,                                    // "md payment-flow.md   guides/"
    Text { line: u32 },                          // snippet rows, capped
    Structure { label: RowLabel },               // id / link rows
    MoreText { omitted: usize },                 // "+ 8 more"
    Escalate { query: String, total: usize },    // "Search all text for "q" — N results"
    Recent { concept_id: String },               // empty-query state
    NoResults { query: String, filters: Vec<String> },
    Indexing,                                    // "indexing…" note under TEXT
}

pub struct PaletteRow {
    pub kind: PaletteRowKind,
    pub title: String,
    pub detail: String,          // path / directory column
    pub hit: Option<waml::search::Hit>,
    pub hidden: bool,            // muted + badge, same rule as results rows
}

pub struct PaletteSectionModel { pub title: String, pub count: usize, pub rows: Vec<PaletteRow> }

/// The blended, sectioned list (spec §Palette): CONCEPTS, DOCUMENTS, TEXT
/// (max 2 rows + MoreText), STRUCTURE, then ALWAYS the Escalate row for a
/// non-empty query. Empty query -> Recent rows. No hits -> NoResults row
/// naming the query and its active filters.
pub fn build_palette_model(
    query: &str,
    hits: &[waml::search::Hit],
    snippets: &dyn Fn(&waml::search::Hit) -> waml::search::Snippet,
    hidden: &std::collections::HashSet<String>,
    recents: &[(String, String)], // (concept_id, title), newest first
    text_status: TextIndexStatus,
) -> Vec<PaletteSectionModel>;
```

- Widget: `PalettePopup` implements `Popup` (see `MenuPopup`): a `TextInput` query box above a row list; Up/Down arm rows, Enter commits the armed row, typing emits a `PaletteQueryChanged(String)` action the shell answers by pushing fresh sections via `pub fn set_sections(&mut self, cx, sections: Vec<PaletteSectionModel>)`. **Every `MouseDown`/`MouseMove` inside the card rect is stamped handled** (`swallows_underlay` in `popup/base.rs` — same rule the menu obeys) so clicks cannot fall through to the underlay. Commit result: `PopupResult` carrying the chosen row (extend `PopupResult` the way `Select` rows do, or carry an index resolved by the opener — follow whichever `select.rs` does).
- `PopupSpec::Palette { tag: LiveId, bounds: Rect }` opens it centred-top like a command palette (x-centred in `bounds`, fixed max width ~560, top offset ~80 — final look is a visual-sign-off item, not a blocker).
- Section mapping: `Names`+`ModelElement` → CONCEPTS (kind label from uml analysis); `Names`+`TextSpan` on a title → DOCUMENTS; `Prose` → TEXT; `Structure` → STRUCTURE; `Model` hits fold into CONCEPTS under their element.

- [ ] **Step 1: Failing tests** on `build_palette_model` in `palette.rs`: sections appear in spec order with counts; TEXT caps at 2 + `MoreText`; escalate row always present for a non-empty query with the total; empty query yields Recent rows; no-hits yields `NoResults` with filter strings; `text_status == Building` yields the `Indexing` row in TEXT while other sections populate (spec §Index lifecycle).

- [ ] **Step 2: Run to verify failure**, then implement model + widget + `root.rs` route (new `ActiveKind::Palette` arm in `route`, `show_at`, `closed_event`/`armed_event` — mirror the `Menu` arms line for line).

- [ ] **Step 3: Run.** `cargo test -p waml-editor popup::palette` — PASS.

- [ ] **Step 4: Gate + commit**

```bash
cargo test --workspace
git add crates/waml-editor/src/popup
git commit -m "feat(editor): palette popup surface with blended sectioned model"
```

---

### Task 11: Palette wiring — hotkey, live query, activation, escalation

**Files:**
- Modify: `crates/waml-editor/src/shortcuts.rs` (`SearchCommand` + `search_command_for`), `crates/waml-editor/src/app/event.rs` (`handle_global_shortcuts`), `crates/waml-editor/src/app/actions.rs` (`observe_popup_results` + palette query actions)

**Interfaces:**
- Produces (`shortcuts.rs`, same platform-primary pattern as `history_command_for`, with the same style of unit tests):

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SearchCommand { OpenPalette, OpenFindStrip, NextHit, PreviousHit }

/// Ctrl+K / Cmd+K palette; Ctrl+F / Cmd+F find strip; F3 / Shift+F3 hits.
/// Alt always disqualifies; F3 needs NO primary modifier. These four chords
/// are checked here, in one function, against everything the editor already
/// claims (Ctrl+Z/Y, V/N/C, ?, T) — the collision audit the spec's risk
/// section asks for IS this function's test module.
pub(crate) fn search_command_for(key: KeyCode, modifiers: KeyModifiers, macos: bool) -> Option<SearchCommand>;
```

- Wiring: `handle_global_shortcuts` maps `OpenPalette` → `App::open_palette(cx)` (build sections from `SearchState` + `SearchSession`-free empty query, `popup_root.show_at(PopupSpec::Palette)`); palette `PaletteQueryChanged(q)` actions (arriving via `observe_popup_results`-adjacent action observation) re-query `SearchState` and `set_sections`; palette commit routes: Concept row → open on canvas + `pending_reveal ModelElement`; Document row → normal open; Text/Structure row → open + `pending_reveal TextSpan`; `MoreText`/`Escalate` row → `open_search_results(cx, query)` (Task 9); Recent row → normal open. `NextHit`/`PreviousHit` handling lands in Task 14; until then `search_command_for` already returns them and `handle_global_shortcuts` maps them to the session advance introduced there — in THIS task, wire `OpenPalette` only and leave F3 mapping to return `false` (not dead: the enum variants are produced and asserted by this task's tests, and consumed by `handle_global_shortcuts` via exhaustive match with a no-op arm carrying a `// Task 14` comment).
- Recents source: most recent distinct concept documents from `self.view_history` (decision 2), newest first, max 8.

- [ ] **Step 1: Failing tests.** `shortcuts.rs`: Ctrl+K/Cmd+K → `OpenPalette`; Ctrl+F/Cmd+F → `OpenFindStrip`; F3/Shift+F3 → `Next/PreviousHit`; wrong platform modifier, extra Alt, and every already-claimed chord (Ctrl+Z, Ctrl+Y, plain V/N/C/T/?) → `None`. App test (`app/tests/` existing pattern): a Ctrl+K `KeyDown` opens the popup root with the palette active; typing updates sections (assert via the palette widget state); Enter on a concept row opens that document's tab.

- [ ] **Step 2: Run to verify failure**, then implement.

- [ ] **Step 3: Run.** `cargo test -p waml-editor` — PASS.

- [ ] **Step 4: Gate + commit**

```bash
cargo test --workspace
git add crates/waml-editor/src
git commit -m "feat(editor): Ctrl+K palette with live query and activation routing"
```

---

### Task 12: Find strip — file creation (widget + counter model)

**Files:**
- Create: `crates/waml-editor/src/find_strip.rs`
- Modify: `crates/waml-editor/src/lib.rs` (module), `crates/waml-editor/src/app.rs` (live design: a hidden `find_strip` overlay row docked above the document body, inside the body column so it overlays the open document only)

**Interfaces:**
- Consumes: `waml::search::{Hit, QueryScope}`, `SearchState` (Task 5), `SearchSession` (Task 9).
- Produces (`pub`):

```rust
/// Pure counter model, unit-testable without widgets.
pub struct FindModel {
    pub query: String,
    pub total: usize,
    pub current: Option<usize>, // 0-based; renders as "3 of 12"
}
impl FindModel {
    pub fn counter_text(&self) -> String; // "3 of 12", "0 results", "" for empty query
    pub fn step(&mut self, forward: bool);  // wraps
}

#[derive(Clone, Debug)]
pub enum FindStripAction { QueryChanged(String), Next, Previous, Close }

pub struct FindStrip { /* Widget: TextInput + counter label + next/prev/close IconButtons */ }
impl FindStrip {
    pub fn open(&mut self, cx: &mut Cx);   // show + focus the input
    pub fn close(&mut self, cx: &mut Cx);
    pub fn is_open(&self) -> bool;
    pub fn set_model(&mut self, cx: &mut Cx, model: &FindModel);
    pub fn action(&self, actions: &Actions) -> Option<FindStripAction>;
}
```

- Enter in the input = `Next`, Shift+Enter = `Previous`, Escape inside the input = `Close` (widget-local; the app-level Escape/session rules come in Task 14).
- **Gate-green justification:** `FindModel` and the widget API are `pub` lib items with unit tests; the live-design registration is usage. App wiring lands in Task 13.

- [ ] **Step 1: Failing tests** on `FindModel`: counter text for empty query, zero results, `3 of 12`; `step` wraps both directions; `current` resets to `None` when the query changes.

- [ ] **Step 2: Run to verify failure**, then implement model + widget (visual styling per the statusbar/document-header patterns; exact look is a sign-off item).

- [ ] **Step 3: Run.** `cargo test -p waml-editor find_strip` — PASS.

- [ ] **Step 4: Gate + commit**

```bash
cargo test --workspace
git add crates/waml-editor/src
git commit -m "feat(editor): find strip widget and counter model"
```

---

### Task 13: Find strip wiring — Ctrl+F, next/previous, canvas dimming

**Files:**
- Modify: `crates/waml-editor/src/app/event.rs` (map `SearchCommand::OpenFindStrip`), `crates/waml-editor/src/app/actions.rs` (handle `FindStripAction`), `crates/waml-editor/src/app.rs` (a `find: Option<SearchSession>` document-scoped session on `App`)

**Interfaces:**
- Consumes: everything from Tasks 5–7, 9, 12. This task adds no new public types.
- Behaviour to wire (spec §Find strip): Ctrl+F opens the strip pre-scoped to the active document (`QueryScope { document: Some(active concept path) }`); `QueryChanged` re-queries and pushes a fresh `FindModel`; on a text surface, all matches get `set_search_highlights` and `Next`/`Previous` call the active view's `reveal(TextSpan)`; on a canvas, the matched node keys go to `set_search_spotlight(Some(keys))` and `Next`/`Previous` call `reveal(ModelElement)` to pan between them (find as a spotlight, not a scroll position); `Close` clears highlights AND the spotlight (`set_search_spotlight(None)`, `clear_search_highlights`) and returns focus to the document. Opening the strip over a tab with no searchable surface (folder view) shows `0 results` — never a crash.

- [ ] **Step 1: Failing app tests** (existing `app/tests/` fixture pattern): Ctrl+F `KeyDown` opens the strip; typing a term from the fixture yields a non-zero counter; `Next` on a canvas tab sets a spotlight on the surface (assert `search_spotlight()` non-empty) and moves the selection; `Close` clears the spotlight and highlights.

- [ ] **Step 2: Run to verify failure**, then implement.

- [ ] **Step 3: Run.** `cargo test -p waml-editor` — PASS.

- [ ] **Step 4: Gate + commit**

```bash
cargo test --workspace
git add crates/waml-editor/src
git commit -m "feat(editor): Ctrl+F find strip with canvas spotlight and reveal stepping"
```

---

### Task 14: Live session — F3/Shift+F3 cross-document traversal, position marking, Esc

**Files:**
- Modify: `crates/waml-editor/src/app/event.rs` (fill the `NextHit`/`PreviousHit` no-op arms from Task 11), `crates/waml-editor/src/app.rs` (a bundle-scoped `session_search: Option<SearchSession>` set by Task 9's `open_search_results` and by palette escalation), `crates/waml-editor/src/search_results_view.rs` (`pub fn set_cursor(&mut self, cx: …, index: Option<usize>)` marks the current row), `crates/waml-editor/src/app/actions.rs`

**Interfaces:**
- Consumes: `SearchSession::advance` (Task 9), `DocView::reveal` (Tasks 6–7), the open path (Task 9).
- Behaviour (spec §Search session): after landing on a hit (results-tab activation or palette commit), the session stays live: every other match in the open document is highlighted (`set_search_highlights` with all of that document's session spans / spotlight for canvas). `F3`/`Shift+F3` advance the session cursor **across document boundaries in results-tab order** — when the next hit is in another document, run the normal open path for it, then `pending_reveal`. The open results tab (if any) mirrors the cursor via `set_cursor`. `Esc` (extend `handle_escape_event` in `app/event.rs`, BEFORE the existing per-view escape so a live session consumes the first Esc) ends the session: clear session, highlights, spotlight, find strip if open, and results-tab cursor mark. A document-scoped find session (Task 13) takes precedence for F3 while the strip is open.

- [ ] **Step 1: Failing app tests**: after `open_search_results` + activating row 0, F3 lands on the session's next hit — including one in a DIFFERENT document (assert the active tab changed and a reveal was applied); Shift+F3 walks back; the traversal wraps; Esc clears the session (highlights empty, spotlight `None`, further F3 does nothing).

- [ ] **Step 2: Run to verify failure**, then implement.

- [ ] **Step 3: Run.** `cargo test -p waml-editor` — PASS.

- [ ] **Step 4: Gate + commit**

```bash
cargo test --workspace
git add crates/waml-editor/src
git commit -m "feat(editor): F3 cross-document traversal and Esc-terminated search session"
```

---

### Task 15: Export-time index — core asset format, CLI export, site boot load

**Files:**
- Create: `crates/waml/src/search/asset.rs`
- Modify: `crates/waml/src/search/mod.rs` (module), `crates/waml/src/search/index.rs` (expose the internals needed to encode/decode via `pub(crate)` accessors used by `asset.rs`), `crates/waml-cli/src/site.rs` (write the asset), `crates/waml-cli/src/commands.rs` (build index during `export site`), `crates/waml-editor/src/browser_boot.rs` + `crates/waml-editor/src/search_state.rs` (try the asset, fall back to local build)

**Interfaces:**
- Produces (`asset.rs`):

```rust
pub const SEARCH_INDEX_SUFFIX: &str = ".search-index";
pub const FORMAT_VERSION: u32 = 1;

/// `"bundle.waml"` -> `"bundle.waml.search-index"`. Total for any bundle name;
/// the index must track whatever `?bundle=<name>` resolved to, never a fixed name.
pub fn index_file_name(bundle_file: &str) -> String;

/// Deterministic, dependency-free, line-oriented text format:
/// line 1: "waml-search-index v1 <fnv1a-of-bundle-bytes>", then documents,
/// entries and postings. FNV-1a is ~10 lines of code — implement it here.
pub fn encode(index: &MemSearchIndex, bundle_hash: u64) -> String;
pub enum AssetError { Version, Corrupt }
pub fn decode(text: &str, expected_bundle_hash: u64) -> Result<MemSearchIndex, AssetError>;
pub fn bundle_hash(pairs: &[(String, String)]) -> u64; // FNV-1a over paths+bytes
```

- CLI: in `commands.rs` `export site`, after reading the bundle: parse + analyze (the CLI already has the analysis pipeline for `waml check` — reuse it), `extract_bundle`, `MemSearchIndex::build`, `asset::encode`, and insert `index_file_name(BUNDLE_FILE) -> bytes` into the `assemble_site` file map for `SiteSource::Static` (in `site.rs`, next to where `BUNDLE_FILE` is inserted — derived from the same constant, so the pair cannot drift). Spec boundary rule holds by construction: the index is built from exactly the pairs the export ships in `bundle.waml`, so search can never leak what the export withheld.
- Editor: where the wasm boot path fetches the bundle (`browser_boot.rs`), also request `index_file_name(name)` for the **same** `BrowserBootSource::Bundle(name)` it just fetched — never a literal; on success AND `decode(text, bundle_hash(pairs))` Ok, seed `SearchState` from the decoded index; on ANY failure, `SearchState::rebuild` as today (decision 10). Native editor keeps building locally — same `SearchState`, one extra constructor `SearchState::from_index(MemSearchIndex)`.

- [ ] **Step 1: Failing tests.** `asset.rs`: encode→decode round-trips query results (same `Vec<Hit>` for three probe queries); wrong version and wrong hash are rejected; `decode` of truncated text is `Corrupt`, never a panic. `index_file_name` derives `bundle.waml.search-index` from `bundle.waml` and `orders.waml.search-index` from `orders.waml`. `site.rs` tests (existing style): a static-site assembly now contains `bundle.waml.search-index` whose first line carries the version and the hash of the bundle bytes; an API-source site does NOT contain it. `browser_boot.rs` test: booting `?bundle=orders.waml` requests `orders.waml.search-index`, not the default name.

- [ ] **Step 2: Run to verify failure**, then implement (remember: core stays dependency-free and wasm-clean — verify `cargo check -p waml --target wasm32-unknown-unknown`).

- [ ] **Step 3: Run.** `cargo test -p waml -p waml-cli -p waml-editor` — PASS. Golden suite (Task 4) must be untouched.

- [ ] **Step 4: Gate + commit**

```bash
cargo test --workspace
git add crates/waml/src/search crates/waml-cli/src crates/waml-editor/src
git commit -m "feat(search): export-built index asset with hash-checked site boot load"
```

---

### Task 16: Typed UI regression scenarios

Per the typed UI regression testing design (`docs/superpowers/specs/2026-08-08-typed-ui-regression-testing-design.md`): scenarios speak the semantic DSL, never `makepad_test` directly; new operations get adapters.

**Files:**
- Modify: `crates/waml-ui-test/src/app.rs` (DSL operations), `crates/waml-ui-test/src/adapters/documents.rs` or a new `crates/waml-ui-test/src/adapters/search.rs` (adapter layer), `crates/waml-ui-test/src/domain.rs` (any new domain names)
- Create: scenario tests in the crate that hosts existing `#[waml_ui_test]` scenarios (follow where `expect_workspace_open` scenarios live today — `crates/waml-ui-test` tests or the editor's UI-test suite)

**Interfaces:**
- Produces (on `WamlApp`, same `execute`-envelope style as `ensure_diagram_open`):

```rust
pub fn open_search_palette(&mut self) -> &mut Self;
pub fn type_search_query(&mut self, query: &str) -> &mut Self;
pub fn expect_palette_sections(&mut self, sections: &[(&str, usize)]) -> &mut Self; // ("CONCEPTS", 3)…
pub fn escalate_to_results_tab(&mut self) -> &mut Self;
pub fn expect_results_grouped_by_document(&mut self, groups: &[(&str, usize)]) -> &mut Self;
pub fn open_find_strip(&mut self) -> &mut Self;
pub fn expect_find_counter(&mut self, text: &str) -> &mut Self; // "1 of 3"
```

- Three scenarios (the spec's Testing section, third bullet): palette sections for a fixture term; results-tab grouping for the same term; find-strip counter in a fixture document. Use the existing `WorkspaceFixture` (`Mini` or whichever fixture carries enough prose — extend the fixture bundle content if `Mini` is too thin, keeping its other scenarios green).

- [ ] **Step 1: Write the three scenarios first** (failing), using only the new DSL methods.
- [ ] **Step 2: Run to verify failure.** `cargo test -p waml-ui-test` (plus the crate hosting scenarios) — FAIL at the first missing DSL method.
- [ ] **Step 3: Implement DSL + adapters** (drive the palette via the same key events the app handles: Ctrl+K, typed characters, Enter; read state through the semantic adapter layer, not widget ids in scenarios).
- [ ] **Step 4: Run.** Scenario tests PASS deterministically (run twice).
- [ ] **Step 5: Gate + commit**

```bash
cargo test --workspace
git add crates/waml-ui-test
git commit -m "test(ui): typed scenarios for palette sections, results grouping, find counter"
```

---

### Task 17: Goals documentation update

Search is currently listed as out-of-scope horizon work in two places; this feature ends the deferral (spec §Documentation updates).

**Files:**
- Modify: `docs/waml/goals/mvp.md` — line 34 lists "search" among exclusions ("Multi-user editing, comments, search, cross-bundle links, or non-UML typed…") and the table row at line 98 routes "Search, collaboration, and non-UML projections" to Beyond UML as horizon work. Remove "search" from the exclusion list; reword the table row so it no longer claims search is horizon work (collaboration and non-UML projections stay), e.g. `| Collaboration and non-UML projections | [Beyond UML](./beyond-uml.md) | These functions are horizon work. |`, and add a short "Bundle search" line to whatever in-scope capability list fits the document's structure (palette, results tab, find-in-document, identical on the exported site).
- Modify: `docs/waml/goals/beyond-uml.md` — lines 20–23 name "the search" as part of the substrate story and "full-text search in a bundle" as future work. Update to state bundle search as landed capability (palette + results + find strip + static-site parity) and keep only genuinely-future search items (cross-bundle search, links between bundles) as horizon work.

- [ ] **Step 1: Make both edits.** Keep each document's voice and structure; do not restructure either file.
- [ ] **Step 2: Verify** no other goals text still claims search is out of scope: `grep -rni "search" docs/waml/goals/`.
- [ ] **Step 3: Gate + commit**

```bash
cargo test --workspace
git add docs/waml/goals/mvp.md docs/waml/goals/beyond-uml.md
git commit -m "docs(goals): bundle search is in scope; horizon items narrowed"
```

---

## Visual sign-off owed (human, after implementation — NEVER a task blocker)

The implementer cannot look at a running GUI. All state logic above is asserted in tests; the following are the *appearance* checks a human runs later, native AND on an exported static site (`waml export site` of `docs/waml`, spec: identical behaviour):

1. Palette placement and typography: Ctrl+K card centred near the top, sections reading `CONCEPTS · 3` style, escalation row pinned last, recents shown on empty query.
2. Muted rendering + `hidden by projection` badge on hidden rows (palette and results tab) is visibly distinct but readable.
3. Results tab: group headers with directory + count, collapse/expand affordance, current-position marker during F3 traversal, header count line including the hidden count.
4. Find strip: thin, docked over the document, does not shift document layout; counter reads `3 of 12`.
5. Canvas dimming: with the find strip active, non-matching nodes dim while matching nodes stay lit, and dimming COMPOSES with selection, hover, and conflict states rather than fighting them (spec risk #4); next/previous visibly pans between lit nodes.
6. Markdown search-match highlight: visible in both light and dark themes, does not occlude the caret or selection.
7. `indexing…` note under TEXT renders sanely (force `TextIndexStatus::Building` in a debug build to view).
8. Exported-site spot check: palette, results tab, find strip, F3, and hidden badges behave identically in the browser build; boot log confirms the shipped `bundle.waml.search-index` was accepted (and that deleting it falls back to a local build).

## Spec coverage self-check (for the reviewer)

- Corpus/fields + dedupe → Tasks 2–3. Query language v1 → Tasks 1, 3. Engine boundary/trait → Task 3. Index lifecycle (open/save) → Task 5; degraded `indexing…` → Tasks 5, 10. Palette → Tasks 10–11. Results tab → Tasks 8–9. Find strip + canvas spotlight → Tasks 7, 12–13. Session + F3 + Esc → Tasks 9, 14. `DocView::reveal` per kind → Tasks 6–7. Activation per document kind → Tasks 9, 11. Projection-hidden hits → Tasks 8–9 (decision 8/9). Static site + export boundary → Task 15. States (empty/no-results/hidden-only/building) → Tasks 10, 8. Testing: unit → Tasks 1–3; golden (early!) → Task 4; typed UI → Task 16; reveal per impl → Tasks 6–7; wasm check → every core task. Docs updates → Task 17. Non-goals: excluded throughout (Global Constraints).
