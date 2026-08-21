# Workspace inventory

What is uncommitted, unmerged, or unfinished in this repository's local
working state, as of 2026-08-21.

None of this is visible from `origin/main`, so none of it is visible to
anyone but the machine it lives on. That is the point of writing it down.

## Branches

65 fully-merged local branches were deleted on 2026-08-21; their commits are
all in `main` and the refs remain in the reflog. 15 branches carry work that
is **not** in `main`:

| Branch | Last commit | Tip subject |
|---|---|---|
| `codex/activity-layout-tuning` | 2026-08-14 | fix(uml): improve activity flow layout |
| `codex/markdown-viewer-hover` | 2026-08-14 | fix(markdown): correct viewer hover feedback |
| `codex/mermaid-visual-tuning` | 2026-08-13 | fix(markdown): tune Mermaid rendering |
| `codex/product-documentation-handbook` | 2026-08-13 | docs: design product documentation handbook |
| `claude/ecstatic-lumiere-5f6793` | 2026-08-12 | fix(editor): tighten folder rows and clear their stale hover |
| `source-surface-unify` | 2026-08-12 | chore(waml-syntax): untrack the local incremental-proptest failure seed |
| `codex/use-case-diagram-rendering` | 2026-08-10 | fix(editor): rasterize ellipse strokes evenly |
| `worktree-edge-zoom-fade` | 2026-08-10 | fix(canvas): hold class-diagram edge ink constant across zoom |
| `worktree-tree-toolbar-compact` | 2026-08-09 | fix(tree-panel): tighten toolbar spacing |
| `fix/layout-diagnostic-messages` | 2026-08-09 | chore: drop stray rustc ICE dumps and ignore the pattern |
| `codex/docs-ui-gwt-architecture` | 2026-08-09 | docs: finish audit cleanup |
| `codex/markdown-editor-emphasis` | 2026-08-08 | fix(editor): preserve default emphasis |
| `spike/surface-seam` | 2026-08-08 | spike(surface-seam): prove the dead surface registry against live shapes |
| `worktree-docs-conformance-harness` | 2026-08-04 | docs: refresh the harness spec's line refs |
| `codex/app-action-coordinator` | 2026-07-27 | refactor(editor): remove shell ownership bypasses |

Most are single-commit UI fixes that were never landed. Each is either worth
landing or worth deleting; sitting unmerged for two weeks is the one option
that has no value. `spike/surface-seam` is explicitly a spike — its findings
are recorded, so the branch itself may be disposable.

## Stashes

16 stashes, oldest 2026-07-19. **Nothing here has been touched** — a stash is
someone's interrupted thought and only its author can judge it. Listed so
they stop being invisible:

| Date | Description |
|---|---|
| 2026-08-12 | WIP on `ci/green-docs-fmt`: journaled atomic save via `host::persist` |
| 2026-08-09 | On `feat/mouse-nav-buttons`: serve-session `## Layout` write on domain-model.md |
| 2026-08-08 | On main: WIP burger-into-slot tests (stashed for implement-plan preflight) |
| 2026-08-08 | On main: WIP burger-into-slot caption edits (pair of the above) |
| 2026-08-05 | On main: stray agent edits to MAIN (33 geometry) |
| 2026-08-05 | On main: stray agent edits to MAIN checkout (36 test-move + layout_geometry) |
| 2026-07-31 | On main: codex integrate preserve main WIP, breadcrumb-tree-reveal |
| 2026-07-31 | On main: CAD linework WIP preserved for HUD integration |
| 2026-07-31 | On `codex/hud-material-salvage`: visual-review artifacts before integrate |
| 2026-07-30 | On main: orders-diagram fixture dirt before overnight sync |
| 2026-07-30 | WIP on main: undo/redo and view history implementation plan |
| 2026-07-26 | On main: move font-size LRU work to worktree |
| 2026-07-25 | autostash |
| 2026-07-21 | On `plan/popup-mechanic-unification`: unrelated fmt drift pre-task7 |
| 2026-07-20 | On `node-editor-polish`: session polish diff (chips/ports/compartments) |
| 2026-07-19 | On main: wip run-native default-fixture edit (superseded by `ccc79e9`) |

Two of these — the 2026-08-05 pair — are **stray agent edits made directly to
the main checkout**, which is the failure mode the "always work in a
worktree" rule exists to prevent. They were stashed rather than discarded, so
whatever they contain is still recoverable.

The 2026-07-19 entry says on its face that it was superseded by `ccc79e9`.
That one is almost certainly safe to drop.

## Worktrees

30 registered worktrees. Their build directories are the machine's dominant
disk consumer: roughly 190GB of git-ignored `target/` output was reclaimed on
2026-08-21 after the disk hit 0 bytes free three times in one session,
corrupting build state each time.

One directory, `.claude/worktrees/fix-classifier-escape-surface`, has **no
`.git` at all** — it is an orphaned husk holding rustc ICE dumps from
2026-08-13. Its build output was cleared; the directory itself remains and can
be deleted outright.
