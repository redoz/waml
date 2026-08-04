# 2026-08-04 issue triage — landing order index

Triage of the sixteen findings in the 2026-08-04 five-domain code-smell section
of `issues.md`. Each was verified against worktree HEAD (`2fdb5ff9`) by an
independent agent; fifteen were approved and have a plan, one was rejected.

Every plan is self-contained and implementable on its own. This file exists for
one reason: **six clusters of plans edit the same files**, and the order within
a cluster is not arbitrary. Read the cluster your plan belongs to before
starting it.

## Rejected

- **Issue 23 — `FieldEdit` serde round-trip turns `Unchanged` into `Clear`.**
  The `Serialize`/`Deserialize` impl described in the issue is real
  (`crates/waml/src/uml/ops.rs:36-55`), but the destructive round-trip is
  latent only: the single serde-containing struct
  (`crates/waml-ops-dto/src/lib.rs:96-97`) carries both
  `#[serde(default)]` and `skip_serializing_if = "FieldEdit::is_unchanged"`, and
  the round-trip test the issue asks for already exists
  (`lib.rs:1014`, all three intents). The suggested debug-panic on serializing
  `Unchanged` would break correct behaviour. No plan written.

## Landing order by cluster

Plans not named in a cluster are standalone and can land in any order.

### Cluster A — `waml-markdown-editor/src/widget.rs`

**20 → 33 → 34 (Tasks 1-2) → 31 (Task 3)**

Issue 20 is the only correctness fix of the four and has the smallest diff, so
it goes first. Issue 33 then renames ten fields under `self.pipeline`, which
would otherwise force 20 to be rewritten. Issue 31 Task 3 adds new tracking
fields and goes last.

### Cluster B — `waml-syntax/src/markdown/inline.rs`

**22 → 28 (task B) → 34 (Task 3)**

Issue 22 changes `parse_inlines`' signature (threads a `depth` parameter)
through every recursive call site — the widest mechanical change, so it lands
first. Issue 21 touches only `reparse.rs` and is independent of this cluster.

### Cluster C — `waml-syntax/src/incremental.rs`

**29 (Task 3) → 28 (tasks A, D) → 35 (Tasks 5-6)**

Issue 29 Task 3 is a localized `unwrap` → `?` conversion. Issue 35 restructures
the whole surrounding function, so it goes last; landing it first would
invalidate the line references in the other two.

### Cluster D — `waml/src/analysis.rs`

**30 → 34 (Task 4)**; 31 (Task 1) independent

Hard collision: issue 30 *moves* `WamlCodeSyntaxSnapshot` into a new
`uml/highlight.rs`, while issue 34 Task 4 *adds a cached field* to that type.
Issue 31 Task 1 edits `impl Display for AnalysisError`, a disjoint region.

### Cluster E — `waml/src/uml/analysis.rs` + `sequence.rs`

**26 → 27 → 29 (Tasks 1, 2, 5) → 35 (Tasks 1-4)**

Issue 26 threads a concept→path index through signatures that 27 and 35 then
restructure. Issue 36's deferred sub-item 9 is folded into issue 35.

### Cluster F — `waml/src/solve/route.rs`

**29 (Task 4) → 36 (Task 4)**

Disjoint regions of one file. Issue 29 gives `Side` a real `Ord`; issue 36
removes the per-edge redundant work.

### Cluster G — `waml-editor/src/editor_session.rs`

**32 → 36 (Task 1)**

Issue 32 removes stale `#[allow(dead_code)]` attributes, which may surface
genuinely dead items needing per-item decisions. Doing that while issue 36 is
mid-move of the 2,390-line test module makes the gate output much harder to
read.

### Cross-plan dependency outside this triage

**28 → `2026-08-04-frontmatter-yaml-alignment.md`.** The YAML plan rewrites
frontmatter parsing and extends `FmValue`, touching every copy that issue 28
tasks A and C collapse into a single authority. Landing the YAML plan first
means implementing its changes three times over.

## Standalone plans

- **21** — reference-use scan (`markdown/reparse.rs` only)
- **24** — okf accessor scans (`okf.rs`, `okf/shell.rs`, `source.rs`)
- **25** — okf `project` expects (`okf.rs`; disjoint from 24's regions)
- **36 Tasks 2, 3, 5** — `snapshot.rs`, `ast.rs`, cross-reference comments

## Notes carried from triage

- Several findings were worse or more nuanced than `issues.md` recorded:
  issue 32 found 165 dead-code allows across 44 files (not ~90); issue 31 found
  a third `format!("{error}")` quarantine site; issue 33 confirmed the reset
  drift already happened in commit `28cbb990`.
- Issue 28's BOM divergence was empirically reproduced and is **fail-safe** —
  BOM'd documents lose incrementality on every edit, a performance bug rather
  than a correctness one.
- Issue 24 corrects the source issue: `Model.nodes` is NOT sorted, so binary
  search is unsound there; `SourceBundle` already has a `by_path` map and needs
  no search at all.
- Issue 25's zero-concept panic is currently reachable only from test callers —
  latent behind a `pub` API, not live.
- Issue 36 approved 5 of 9 sub-items; sub-items 1, 3, 4 are deferred because
  their stated "when next touched" trigger has not fired, and sub-item 9 is
  folded into issue 35.
