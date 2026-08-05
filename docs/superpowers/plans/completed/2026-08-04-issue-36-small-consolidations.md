# Issue 36 — Smaller consolidations (approved subset)

## Context

Issue 36 is a P3 grab-bag of nine deferred, opportunistic consolidations, explicitly framed
as "when next touched". All nine were re-verified against HEAD (2fdb5ff9) on 2026-08-04.

Five sub-items are approved for action now, because each is small, mechanically verifiable,
and carries a real cost today rather than only on a hypothetical future edit:

- **Sub-item 2** — move `editor_session.rs`'s 2,390-line inline `mod tests` to a child file.
- **Sub-item 5** — collapse `MarkdownSyntaxQueries`' eight hand-maintained
  `Arc<[T]>` + `HashMap<SyntaxIdentity, usize>` field pairs behind one `IndexedByOwner<T>`.
- **Sub-item 6** — delete the three `AstSlots` aliases that promise semantics they do not
  have, and fix `AstSlots::list`'s quadratic scan.
- **Sub-item 7** — remove the per-edge redundant work in `solve/route.rs`.
- **Sub-item 8** — cross-reference the two hand-rolled minimal-diff implementations so the
  third copy triggers a merge.

The other four (1, 3, 4, 9) are deferred; see "Deferred sub-items" at the end.

## Verdict evidence (re-verified at HEAD)

**Sub-item 2 — APPROVE.** `crates/waml-editor/src/editor_session.rs` is 3,429 lines;
`#[cfg(test)] mod tests {` starts at :1038-1039, so 2,390 lines are tests and 1,038 are not.
The precedent is live: `crates/waml-editor/src/app.rs:1209-1210` is `#[cfg(test)]\nmod tests;`
with `crates/waml-editor/src/app/tests/` holding `mod.rs`, `menus.rs`, `navigation.rs`,
`shell.rs`, `workspace.rs`. The move is a cut-and-paste; a child module keeps `use super::*`
and private-item access, so no visibility changes are needed.

**Sub-item 5 — APPROVE.** `crates/waml-syntax/src/markdown/snapshot.rs:143-160` — eight
`Arc<[T]>` fields (`links`, `images`, `headings`, `lists`, `cells`, `html`, `fenced`,
`islands`) each paired with a `HashMap<SyntaxIdentity, usize>` (`*_by_owner`, :153-160).
Eight accessors at :185-224 are the identical three-line
`self.X_by_owner.get(&owner).and_then(|&i| self.X.get(i))`. Construction at :535-565 runs
`identity_map(&collected.X, |value| value.owner)` seven times plus one hand-rolled variant
for `island_by_owner` (:542-546), which exists only because `islands` is already an
`Arc<[T]>` at that point rather than a `Vec`. `spans`, `diagnostics`, `entities`,
`filtered_ranges`, and `backlinks` have no owner index and are NOT part of this change —
`entities` in particular already shipped without one, which is the evidence that the pairing
is a hand-maintained convention rather than an enforced invariant.

**Sub-item 6 — APPROVE.** `crates/waml-syntax/src/ast.rs:37-42` — `optional_node` and
`optional_token` are one-line forwards to `required_node`/`required_token`; :48-50
`recovery` is a one-line forward to `required_node`. No distinction exists. `AstSlots` has
exactly one consumer in the workspace: `crates/waml-syntax/tests/red_ast.rs` (:195-196,
:205, :208, :211). Every other `.recovery()` / `.list(...)` hit in the grep is a different
API (`SyntaxNode::recovery`, `MarkdownSyntaxQueries::list`, `okf::shell::list`).
`AstSlots::list` (:43-47) calls `self.syntax.child_at(index)` per index, and `child_at` is
`self.children().nth(i)` (`crates/waml-syntax/src/red.rs:359-361`) — O(n) per element, so
`list(range)` is O(n·k), quadratic on a full-width range.

**Sub-item 7 — APPROVE.** `crates/waml/src/solve/route.rs`, inside the per-edge loop of
`route_keyed_with` (:146-180):
- :152-163 clones the whole masked obstacle list per edge (`.cloned().collect()`), and
  `Obstacle::id` is a `BoxId` carrying `String`s.
- :167-170 builds `inflated: Vec<Rect>` from those obstacles; `build_ovg` (:396-400) then
  builds the *same* `inflated` list again from the *same* slice on the very next line.
  Two identical inflations of every obstacle, per edge.
- `nudge` (:875-907) clones `route.source` and `route.target` (`Seg.src`/`Seg.tgt` are
  `String`, :863-870) for every interior segment of every route.

The P-3 comment at :133-137 already hoisted the candidate list build out of the loop, so
this is finishing a refactor that is half-done, not starting one.

**Sub-item 8 — APPROVE.** `crates/waml/src/analysis.rs:1449` `single_text_change` and
`crates/waml/src/edit/reversible.rs:150` `text_splice` are two independent common-prefix /
common-suffix minimal-diff implementations, both with tests, both correct. Two instances is
below the merge threshold; the cost today is that a third author will not know either
exists. Two doc comments fix that for zero risk.

## Ordering / conflict flags (do not fold into this plan)

- **`route.rs` vs issue 29.** The approved draft
  `docs/superpowers/plans/drafts/issue-29-panics-and-catchalls.md` Task 4 gives `Side` a real
  `Ord` and deletes the `u8` round-trip at `route.rs:1002-1018`. That is a disjoint region
  from this plan's :146-180 and :863-907, but both edit the same file. **Land issue 29 first**
  and rebase this plan's route.rs task onto it; do not run them concurrently.
- **`uml/analysis.rs` (sub-item 9) belongs to issue 35.**
  `docs/superpowers/plans/drafts/issue-35-analyze-incremental-decomposition.md` decomposes
  `analyze` at `analysis.rs:248-700` — that IS the "next touch". Sub-item 9's field/getter
  discipline and the unique-basename triplication should be folded into issue 35's scope, not
  duplicated here.
- **`snapshot.rs`.** No approved draft currently edits
  `crates/waml-syntax/src/markdown/snapshot.rs`; the markdown-syntax-platform plan that
  created it is already landed. Safe to run standalone.

## Design decisions

1. **`IndexedByOwner<T>` lives next to its only consumer**, in
   `crates/waml-syntax/src/markdown/snapshot.rs`, private to the module. It is not a general
   utility until a second module wants it — one consumer does not justify a public type in
   `waml-syntax`'s API surface.
2. **No public API change to `MarkdownSyntaxQueries`.** Every existing accessor
   (`headings()`, `heading(owner)`, `list(owner)`, …) keeps its exact signature and return
   type. The struct's fields are already private, so this is invisible to every caller. That
   is what makes the change safe enough to do without a trigger.
3. **`spans`, `diagnostics`, `entities`, `backlinks` are left alone.** They have range
   filters or different key shapes, not owner indexes. Forcing them into `IndexedByOwner`
   would be the abstraction reaching past its evidence.
4. **Delete the `AstSlots` aliases rather than give them real semantics.** A name that
   promises optionality and delivers none is worse than no name; if a caller later needs a
   real distinction it can be added then, with a caller to define it against.
5. **`route.rs` changes must be byte-identical in output.** Every edit is a hoist, a borrow,
   or an `Arc`/reference swap — no reordering, no changed comparison. The existing route
   tests are the proof obligation; if any golden route moves, the change is wrong.
6. **Sub-item 8 gets comments, not a merge.** The two implementations differ in signature and
   in return type; unifying them now would invent a shared abstraction for two call sites.

## Tasks

### Task 1: Move `editor_session`'s inline test module to a child file

Files:
- `crates/waml-editor/src/editor_session.rs` (edit)
- `crates/waml-editor/src/editor_session/tests.rs` (new)

Steps:
1. Create `crates/waml-editor/src/editor_session/tests.rs`. Move the entire body of the
   `mod tests { … }` block at `editor_session.rs:1038` (everything between the opening brace
   and the file's final closing brace) into it, dedented one level. Keep `use super::*;` and
   all other `use` lines exactly as they are.
2. Replace the block in `editor_session.rs` with:
   ```rust
   #[cfg(test)]
   mod tests;
   ```
   matching `app.rs:1209-1210`.
3. Do not touch a single line of test content. If the diff shows any change other than the
   dedent, revert and redo.

Verify: `cargo test -p waml-editor` — the test COUNT must be identical to the pre-change run.
Capture the count before starting.

### Task 2: Collapse `MarkdownSyntaxQueries`' owner indexes behind `IndexedByOwner<T>`

Files:
- `crates/waml-syntax/src/markdown/snapshot.rs` (edit)

Steps:
1. Add a private type near the top of the module:
   ```rust
   /// An immutable slice plus an owner -> index map built from it. Replaces the
   /// hand-maintained `Arc<[T]>` + `*_by_owner: HashMap<SyntaxIdentity, usize>` field
   /// pairs, so the two can no longer drift apart.
   struct IndexedByOwner<T> {
       items: Arc<[T]>,
       by_owner: HashMap<SyntaxIdentity, usize>,
   }

   impl<T> IndexedByOwner<T> {
       fn new(items: impl Into<Arc<[T]>>, owner: impl Fn(&T) -> SyntaxIdentity) -> Self { … }
       fn get(&self, owner: SyntaxIdentity) -> Option<&T> {
           self.by_owner.get(&owner).and_then(|&i| self.items.get(i))
       }
       fn iter(&self) -> impl Iterator<Item = &T> { self.items.iter() }
       fn as_slice(&self) -> &Arc<[T]> { &self.items }
   }
   ```
   `new` builds `by_owner` by enumerating `items` — the same body as the existing
   `identity_map` helper, which becomes dead and must be deleted.
2. Replace the sixteen fields at :143-160 with eight:
   `links`, `images`, `headings`, `lists`, `cells`, `html`, `fenced`, `islands`, each
   `IndexedByOwner<T>`. Leave `spans`, `diagnostics`, `entities`, `backlinks`,
   `filtered_ranges` untouched.
3. Rewrite the eight owner accessors (:185-224) as one-liners delegating to
   `self.X.get(owner)`. Rewrite `links()`, `headings()`, `lists()`, `images()` as
   `self.X.iter()`. **Signatures do not change.**
4. In the builder (:535-565) replace the seven `identity_map(...)` calls and the hand-rolled
   `island_by_owner` block with eight `IndexedByOwner::new(collected.X, |v| v.owner)` calls.
   `islands` is already an `Arc<[…]>` at that point — `Into<Arc<[T]>>` accepts it unchanged,
   which is why the one-off variant disappears.
5. Fix up any other in-file reader of a raw field (grep the file for `self.islands`,
   `self.fenced`, etc. outside the accessors) to use `.as_slice()` / `.iter()`.

Verify: `cargo test -p waml-syntax` (covers `tests/markdown_queries.rs`,
`tests/markdown_conformance.rs`, `tests/properties.rs`, all of which exercise the owner
lookups) and `cargo test -p waml-markdown-editor` (`presentation/compile.rs:431` calls
`.list(owner)`). `cargo clippy --workspace --all-targets` must be clean — no
`dead_code` left behind by the removed `identity_map`.

### Task 3: Delete the `AstSlots` aliases and de-quadratic `list`

Files:
- `crates/waml-syntax/src/ast.rs` (edit)
- `crates/waml-syntax/tests/red_ast.rs` (edit)

Steps:
1. Delete `optional_node` (:37-39), `optional_token` (:40-42), and `recovery` (:48-50) from
   `AstSlots`.
2. Rewrite `list` (:43-47) to walk children once instead of calling `child_at` per index:
   ```rust
   pub fn list(&self, indices: Range<usize>) -> Vec<SyntaxElement<L>> {
       let start = indices.start;
       self.syntax
           .children()
           .skip(start)
           .take(indices.len())
           .collect()
   }
   ```
   Note the behaviour difference to preserve deliberately: the old `filter_map` silently
   dropped out-of-range indices; `skip`/`take` yields the same result for an in-range prefix
   and the same (shorter) result when the range runs past the end. Assert this in the test
   below rather than assuming it.
3. In `crates/waml-syntax/tests/red_ast.rs`, change `.optional_token(Self::VALUE)` (:205) to
   `.required_token(...)` and `.recovery(Self::RECOVERY)` (:211) to `.required_node(...)`.
   `.list(Self::ITEMS)` (:208) is unchanged.
4. Add a test to `red_ast.rs` asserting `list` over a range that extends past the child count
   returns exactly the children that exist, and that `list` over an empty range is empty.

Verify: `cargo test -p waml-syntax`. `cargo clippy --workspace --all-targets -- -D warnings`
proves no remaining caller of the deleted methods anywhere in the workspace.

### Task 4: Remove the per-edge redundant work in `solve/route.rs`

**Do this task only after issue 29 has landed**, and rebase onto it first.

Files:
- `crates/waml/src/solve/route.rs` (edit)

Steps:
1. Capture the baseline first: run `cargo test -p waml` and record it green. Every step below
   must keep every route test byte-identical.
2. Kill the double inflation. `build_ovg` (:396-400) computes `inflated` from `obstacles`;
   `route_keyed_with` (:167-170) computes the identical list. Change `build_ovg` to
   `fn build_ovg(obstacles: &[Obstacle], inflated: &[Rect], src: Rect, tgt: Rect)` and pass
   the caller's list in, computing it once at :167 before the `build_ovg` call. Update the
   test call sites (`route.rs:2117`, `:2279` and any other) to compute and pass it.
3. Avoid the per-edge clone of the obstacle list (:152-163). Build a `Vec<&Obstacle>` of the
   masked candidates instead of `.cloned().collect()`, or — simpler and equivalent — build a
   `Vec<usize>` of surviving indexes into `all_obstacles`. Whichever form is chosen,
   `build_ovg` and the `inflated` build must iterate the same elements in the same order, so
   the routes cannot move.
4. Stop cloning endpoint ids per segment in `nudge` (:875-907). `Seg.src`/`Seg.tgt` exist only
   for the tie-break sort at the top of `sweep`. Replace `src: String` / `tgt: String` with
   borrows of the route's `source`/`target` (`Seg<'a>` with `&'a str`), or with the route
   index pair if the sort can key on `routes[ri].source` directly. The sort comparison
   (`.then(p.src.cmp(&r.src)).then(p.tgt.cmp(&r.tgt))`) must produce the identical ordering —
   `&str` `Ord` matches `String` `Ord`, so a borrow is a safe swap. Note `sweep` currently
   takes `chan` by value while `routes` is `&mut`; if the borrow fights that, collect the
   comparison keys before the mutation loop rather than reintroducing the clone.
5. Re-run the full `waml` suite. If ANY route golden changes, the refactor introduced a
   reorder — revert that step and redo it.

Verify: `cargo test -p waml` — identical pass set to the baseline captured in step 1. Then
`cargo test --workspace`.

### Task 5: Cross-reference the two minimal-diff implementations

Files:
- `crates/waml/src/analysis.rs` (edit)
- `crates/waml/src/edit/reversible.rs` (edit)

Steps:
1. Above `single_text_change` (`analysis.rs:1449`) add:
   ```rust
   /// Minimal single-splice diff over `SourceText`, for the reparse path.
   ///
   /// A second, independent common-prefix/common-suffix diff exists at
   /// `crate::edit::reversible::text_splice` — that one works on `&str` and produces a
   /// `TextSplice` for undo. Both are verified correct and are kept separate on purpose:
   /// two instances is below the threshold at which a shared abstraction pays for itself.
   /// **If you are about to write a third, merge all three instead.**
   ```
2. Add the mirror-image comment above `text_splice` (`edit/reversible.rs:150`), pointing at
   `crate::analysis::single_text_change` and carrying the same "third copy triggers a merge"
   instruction.
3. No code changes in this task.

Verify: `cargo doc -p waml --no-deps` builds without an intra-doc-link warning (both paths
must resolve). `cargo clippy --workspace --all-targets -- -D warnings`.

## Deferred sub-items (REJECT — trigger has not fired)

- **Sub-item 1 — first-match-wins action router**
  (`crates/waml-editor/src/class_diagram_view.rs:488-841`, file is 1,894 lines). The stated
  trigger is "when the next branch is added". It has not been added. Splitting a
  ~350-line dispatch chain into `route(actions) -> Vec<Intent>` plus a dispatcher is a real
  design change with no test harness behind it today, and the one known defect
  (`camera_changed` at :686 needing to co-exist with a click in the same batch) is already
  handled. Do it with the branch that forces it, so the new branch is the test case.

- **Sub-item 3 — split `crates/waml/src/uml/syntax/parser.rs`** (4,734 lines, confirmed) and
  the parallel extraction of `analysis.rs`'s `declared_*` block. The issue's own assessment is
  that this is not a god module: 87 free functions, clean seams, no shared state. A purely
  mechanical 4,700-line file split buys nothing measurable, destroys `git blame` continuity
  across the whole parser, and conflicts with anything in flight. No trigger.

- **Sub-item 4 — split `crates/waml-markdown-editor/src/layout/engine.rs`** (2,871 lines,
  confirmed). The seams named are real, but the stated trigger is "before the next feature
  lands here". No markdown-layout feature is queued. Splitting it now means paying the merge
  cost twice — once for the split, once when the feature arrives and reshapes the seams
  anyway. Do it as the first commit of that feature.

- **Sub-item 9 — `Analysis` field/getter discipline and the triplicated unique-basename
  disambiguation** (`crates/waml/src/uml/analysis.rs:21-33` confirmed: six `pub` fields then
  five private ones; basename variants at :172-176, `lower.rs:137-144`, `lower.rs:1424-1428`).
  Not rejected on merit — rejected as a duplicate. `analysis.rs` IS being touched, by the
  approved `issue-35-analyze-incremental-decomposition` draft, and issues 27 and 30 also land
  there. Fold sub-item 9 into issue 35's scope rather than opening a competing edit to the
  same file.
