# Issue 34 — Per-frame and per-keystroke hot-path costs

**Verdict: APPROVE — all five sub-items verified present at HEAD (2fdb5ff9).**

## Context

Five independent hot-path costs across the markdown editor draw loop, the inline
markdown parser (LSP keystroke path), analysis code-span queries, and edit-batch
invalidation classification. Each is a localized perf fix with no behavior change;
each task below is independently landable and gate-able.

## Ordering / conflict flags (read before starting any task)

- **Task 4 has a HARD dependency on issue 30.**
  `issue-30-analysis-uml-highlighting.md` MOVES `WamlCodeSyntaxSnapshot` out of
  `crates/waml/src/analysis.rs` into a new `crates/waml/src/uml/highlight.rs`.
  Task 4 below adds a cached `spans` field to that exact type. **Land issue 30
  first**, then apply Task 4 against the type in its new home. Do not run them
  concurrently.
- **Task 3 shares `inline.rs` with issues 22 and 28.** Issue 22 adds a nesting
  depth cap (threads a `depth` parameter through `parse_inlines`); issue 28
  records a `resolution` on `BracketMatch` and deletes `parse_link`'s
  re-derivation. Task 3 here is strictly the indexing/quadratic work — do NOT
  add or alter depth caps, and do NOT restructure bracket matching. Suggested
  order for that file: 22, then 28, then this Task 3, rebasing each time.
- **Tasks 1-2 share `waml-markdown-editor/src/widget.rs` with issues 20, 31,
  and 33.** Issue 33 in particular moves the pipeline fields these tasks read
  into a `LayoutPipeline` struct. **Land 20, then 33, before Tasks 1-2**, or
  expect to rewrite every `self.<field>` reference in the diff.

## Verdict evidence (per sub-item, verified 2026-08-04)

1. `crates/waml-markdown-editor/src/widget.rs:851-856` — `draw_walk_with_session`
   gets `(base_commands, plan)` from `cached_draw_commands` (cache added in
   28cbb990) but then, every frame, allocates a fresh
   `Arc<[_]>` of `command.translated(content_origin)` over the whole command list,
   even on a pure cache hit with unchanged origin.
2. `widget.rs:1178-1191` — `paint_text` does
   `layout.glyph_clusters().iter().find(|c| c.id == paint.id)` once per text
   paint operation, so a frame with R text runs costs O(R²) cluster scans.
3. `crates/waml-syntax/src/markdown/inline.rs:297,323,569` — `parse_inlines`
   does `strikethrough.iter().find(|p| p.open == at)`,
   `emphasis.iter().find(...)`, and `brackets.iter().find(...)` inside the
   per-byte `while at < end` loop → O(bytes × pairs). Additionally
   `code_spans`/`angle_spans` (lines 1086, 1160, defined 1260/1286) are
   recomputed from scratch inside helpers invoked at every recursion level of
   `parse_inlines`, and line 1382 builds a `format!("<{value}>")` String per
   raw-HTML candidate. Adversarial inline runs (e.g. thousands of `*` or `[`)
   make this quadratic-or-worse inside the LSP keystroke path.
4. `crates/waml/src/analysis.rs:141-165` — `OkfAnalysis::code_spans(owner, ...)`
   linearly scans `self.markdown.documents().values()` probing every snapshot's
   queries to validate an owner whose snapshot is directly addressable; and
   `WamlCodeSyntaxSnapshot::code_spans()` (line 341-352) re-walks the whole
   syntax tree, sorts, and dedups on every call.
5. `crates/waml/src/edit/batch.rs:197-218` — `claimed_id` runs a full
   `parse_markdown` of the document just to read the frontmatter `type` field;
   `invalidations` (220-282) calls it for every removed/inserted/renamed path,
   and its rename matching is an O(removed × inserted) `position` loop. A
   directory move of N documents does N discarded full parses.

## Design decisions

- **D1 (sub-item 1):** Prefer the issue's suggestion — pass `content_origin`
  into the paint functions and drop the translated-copy allocation entirely,
  rather than caching the translated list keyed on origin (origin changes on
  every scroll, so a keyed cache would still miss constantly).
- **D2 (sub-item 2):** Build a `HashMap<ClusterId, usize>` (or sorted index)
  once per layout snapshot, stored alongside/inside `LayoutSnapshot` or a
  per-frame local built once in `draw_walk_with_session`, not per paint call.
- **D3 (sub-item 3):** Index emphasis/strikethrough/bracket pairs by open
  offset (`HashMap<usize, ...>` or a sorted vec + cursor advanced monotonically
  with `at`) — the loop only ever queries `open == at` with `at`
  non-decreasing, so a cursor over a sorted vec is the cheapest fix. Compute
  `code_spans`/`angle_spans` once at the top-level `parse_inlines` entry and
  pass the protected spans down through recursion. Replace the
  `format!("<{value}>")` probe with a direct check that avoids allocating.
- **D4 (sub-item 4):** `code_spans(owner, ...)` should resolve the owning
  snapshot directly (owner → document lookup) instead of scanning all
  documents; compute spans once when the code-syntax snapshot is attached
  (`attach_code_syntax`) and store `Arc<[WamlCodeSpan]>` in
  `WamlCodeSyntaxSnapshot` so `code_spans()` is a clone.
- **D5 (sub-item 5):** `claimed_id` needs only the frontmatter fence; scan the
  raw text for the closed frontmatter block (or consult the shell cache if one
  exists for the path) instead of a full markdown parse. Keep the rename
  matching as-is unless trivial to index (it is Arc-ptr identity matching;
  a `HashMap<*const String, usize>` over inserted makes it linear). Comment the
  intent either way.
- **No behavior changes anywhere.** Every task must leave outputs
  byte-identical; parser tasks are covered by the existing proptest suite.

### Task 1: Stop re-translating the draw-command list every frame

Files: `crates/waml-markdown-editor/src/widget.rs`

- In `draw_walk_with_session` (~line 851), delete the
  `base_commands.iter().map(|c| c.translated(content_origin)).collect::<Arc<[_]>>()`
  allocation.
- Thread `content_origin: DVec2` into the per-command paint path: the layer
  loop indexes `commands[index]` — switch to `base_commands[index]` and apply
  the translation at use sites (`paint_command`, `build_text_paint_operations`
  input rect, `paint_evidence.record_command` if it consumes absolute rects —
  check whether evidence expects translated or base coordinates and keep it
  consistent with today's output).
- If `DrawCommand::translated` is cheap per-command, an acceptable minimal form
  is translating a single command on the stack inside the loop instead of
  collecting the whole list; prefer whichever keeps the diff smallest while
  removing the per-frame Arc allocation.
- Tests: existing widget/paint-evidence tests must pass unchanged
  (`cargo test -p waml-markdown-editor`). Visual verify the editor scrolls and
  paints identically (gutter offset + scroll).

### Task 2: Index glyph clusters once instead of linear find per text command

Files: `crates/waml-markdown-editor/src/widget.rs` (and the `LayoutSnapshot`
type's home if the index lives there)

- Replace the `layout.glyph_clusters().iter().find(|c| c.id == paint.id)` in
  `paint_text` (~1185) with a lookup against an index built once per snapshot.
- Preferred: add a lazily-built `HashMap<Id, usize>` accessor on
  `LayoutSnapshot` (`OnceCell` keyed by nothing — the snapshot is immutable),
  or build the map once per frame in `draw_walk_with_session` and pass it down
  with `TextGlyphPaint`/as a parameter.
- Tests: `cargo test -p waml-markdown-editor`; no behavior change.

### Task 3: De-quadratic parse_inlines pair matching and protected-span recomputation

Files: `crates/waml-syntax/src/markdown/inline.rs`

- Scope guard: indexing only — do not touch nesting-depth logic (issue 22).
- Replace the three `iter().find(|p| p.open == at)` / `matched.start == at`
  scans (lines 297, 323, 569) with monotone cursors: the pair vectors are (or
  can be) sorted by open offset and `at` only increases, so keep an index per
  vector and advance it past entries with `open < at`; match when
  `open == at`. (Verify sortedness of `emphasis_pairs`/`strikethrough_pairs`/
  bracket output; sort once if not guaranteed.)
- Hoist `code_spans(source, start, end)` + `angle_spans(...)` (call sites 1086,
  1160) so they are computed once for the top-level range and passed down as
  `&[Range<usize>]` (sliced/filtered per recursion range) instead of rescanned
  per recursion level. Keep results identical: a recursion level today computes
  spans over its sub-range only — confirm sub-range recomputation equals
  filtering the top-level spans to the sub-range (backtick spans cannot cross
  the delimiters that created the sub-range; verify with tests before relying
  on it — if not provably identical, keep per-level computation but memoize).
- Replace `format!("<{value}>")` (~1382) raw-HTML candidate probing with a
  non-allocating check against the source slice.
- Tests: full `cargo test -p waml-syntax` including the markdown proptest
  suite (`tests/properties.rs`) — incremental-vs-full equivalence is the
  safety net. Add a regression test with an adversarial input (e.g. 10k `*`
  characters, 10k `[`) asserting parse completes and, ideally, add it to any
  existing perf/fuzz fixture location.

### Task 4: Direct owner lookup and precomputed code spans in analysis

Files: `crates/waml/src/analysis.rs`

- `OkfAnalysis::code_spans(owner, content_range)` (~141): the owning snapshot
  is already recorded — `self.code_syntax.get(&owner)` yields
  `WamlCodeSyntaxSnapshot { document, .. }`. Reorder: fetch
  `self.code_syntax.get(&owner)` first, then fetch the markdown snapshot by
  `syntax.document` via `self.markdown.document(...)`, and validate that
  document's island/fence for `owner` — eliminating the scan over all markdown
  documents.
- `WamlCodeSyntaxSnapshot::code_spans()` (~341): compute the walked/sorted/
  deduped `Arc<[WamlCodeSpan]>` once at construction (in `attach_code_syntax`
  or wherever the snapshot is built) and store it as a field
  (`spans: Option<Arc<[WamlCodeSpan]>>`); `code_spans()` becomes a clone.
- Check `document_code_spans` (~167) picks up the cached field for free.
- Tests: `cargo test -p waml` — existing analysis/code-span tests must pass
  unchanged.

### Task 5: Frontmatter-only read in batch invalidation classification

Files: `crates/waml/src/edit/batch.rs`

- `claimed_id` (~197): replace the full `parse_markdown` with a cheap
  frontmatter-fence extraction — scan the head of the text for the closed
  `---` frontmatter block and parse only that (reuse an existing frontmatter
  scanner if one exists in `crate::frontmatter` / the shell cache; otherwise a
  small local scanner mirroring the block-parser's frontmatter rules).
  If exact parity with the markdown parser's frontmatter recognition is not
  achievable cheaply, an acceptable fallback is caching `claimed_id` per
  `(path, Arc::as_ptr(text))` across the batch so a directory move parses each
  document at most once instead of once per event classification.
- `invalidations` (~236-254): replace the O(removed × inserted) rename
  matching with a `HashMap<*const String, usize>` (Arc ptr → inserted index)
  built once, preserving current match semantics (first match by pointer
  identity).
- Add a comment stating the intent: classification only needs the frontmatter
  `type` and the concept id, never the full tree.
- Tests: `cargo test -p waml` — the Step/Batch op tests (40ecc384) cover the
  invalidation seam; add a test that a rename (same Arc text, new path) still
  classifies as `Renamed` with correct ids.
