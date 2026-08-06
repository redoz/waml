# Groups as a soft cohesion force in the stress default layout

## Context

Adding groups to a class diagram wrecks its layout today. Two separate defects, both
reproduced on a scratch fixture (the `groups` fixture in tree declares zero
relationships, so nothing in CI exercises groups + edges together):

1. **Groups + edges, no `## Layout` section — groups vanish and members scatter.**
   `use_stress_default` (`crates/waml-editor/src/scene.rs:265`) routes on
   `diagram.layout.is_empty()` alone, so a grouped diagram takes the
   `stress_default` path, which is group-blind by construction: it emits
   `groups: Vec::new()` (`scene.rs:479`) and routes with an empty boxes slice
   (`scene.rs:475`). Probe output on a grouped-plus-related fixture — no group
   rows at all, members of `Users` and `Billing` interleaved:

   ```
   node  account   546.7,174.3      node  invoice  0.0,87.9
   node  customer  309.3,263.1      node  order    235.2,0.0
   ```

2. **Groups + `## Layout` — edge-blind strip.** `geometry.rs` drops every
   constraint whose endpoints are not siblings (`solve_with_rects_labeled:647`),
   and `solve_cluster` places only what constraints say; unconstrained boxes each
   become their own component packed left-to-right at y=0 (`geometry.rs:289-310`).

Groups are **not layout**. A group is a soft force that wants its members near each
other — but not at all cost: an edge to an outside node must still be able to pull a
member away. So the routing rule stays exactly as it is (stress iff
`diagram.layout.is_empty()`), and the fix is to teach the stress solver about group
membership. Defect 2 is out of scope: authoring a `## Layout` section is an explicit
request for the constraint solver.

Decisions taken with the user:

- **Frames**: keep today's rule. A group draws chrome only for `Shape::Frame`
  (`canvas/class/render/groups.rs:12`), which only a `with frame` layout statement
  produces — so in the stress path groups stay invisible. Cohesion is a placement
  force; hulls are still emitted (as `Shape::Shrink`) so the show-hidden dashed
  overlay and containment-aware routing can consume them.
- **Hull overlap**: push apart. After the solve, sibling hulls that overlap are
  separated by translating whole groups, and non-member nodes are pushed out of
  hulls they landed inside. Without frames on screen the clusters still have to
  *read* as clusters.

Outcome: a grouped diagram with relationships lays out with members clustered,
edges still shaping the result, and no cluster tangled into another.

## Approach

SMACOF already runs off a full target-distance matrix `dist` with weights `w`
(`crates/waml/src/solve/stress.rs:314-331`). Cohesion drops straight in: co-members
get a shorter target distance and a heavier weight. It is soft for free — every
other pair keeps its own stress term, so a strong edge can outvote the group.

### Task 1 — `GroupSpec` and a grouped entry point in `stress.rs`

`crates/waml/src/solve/stress.rs`

Add:

```rust
pub struct GroupSpec {
    /// Indices into `ids` of every member, including members of nested children.
    pub members: Vec<usize>,
    /// Nesting depth; 0 is top level. Deeper groups bind tighter.
    pub depth: u8,
}

pub fn layout_grouped(
    ids: &[BoxId],
    sizes: &[Size],
    edges: &[(usize, usize)],
    groups: &[GroupSpec],
    cfg: &StressConfig,
) -> (Vec<Rect>, Vec<Rect>)   // (node rects, one hull rect per group, in order)
```

Keep `pub fn layout` as a thin wrapper: `layout_grouped(.., &[], ..).0`. All existing
callers and `tests/stress_golden.rs` keep passing unchanged — that is the regression
guard for "groups change nothing when there are none".

New `StressConfig` fields with defaults: `group_len` (ideal co-member separation,
start at `edge_len * 0.75`), `group_weight` (co-member weight multiplier, start at
`4.0`), `hull_pad` (padding from member bbox to hull, start at `SolveConfig::default().margin(Margin::Medium)`).

### Task 2 — cohesion inside the solve

Same file. Three changes, all in the `layout_grouped` body / `component_layout`:

1. **Augmented adjacency.** Build a co-membership relation alongside the real edges
   (for each group, its members form a clique). Feed `edges ∪ comembership` to
   `components()` so a group can never be split across two independently
   shelf-packed components, and to `bfs_hops` so `dist` is defined for every pair in
   the merged component. Group sizes here are small; the O(k²) clique is fine.
2. **Target distance.** Compute `dist` from hops as today, then for every co-member
   pair override to `min(dist, group_len + half_extent(a) + half_extent(b))`.
   A pair that is *both* edge-adjacent and co-member keeps the tighter of the two.
3. **Weight.** Multiply `w[a][b]` for co-member pairs by
   `group_weight.powi(depth as i32 + 1)`, using the *deepest* group that contains
   both. `wsum` is already derived from `w`, so it follows.

Determinism is preserved: no RNG, fixed iteration order, groups processed in input
order.

### Task 3 — hull computation and separation pass

Same file, after the existing `remove_overlaps` call in `layout_grouped`.

- `hull(group) = bbox(members) grown by hull_pad`.
- Bounded loop (cap 4 iterations, then accept):
  1. Recompute all hulls.
  2. Deepest-first, then by group index: for each overlapping pair of hulls that are
     not in an ancestor/descendant relation, translate *every member rect* of the
     later group along the axis of smaller overlap until the hulls clear by `cfg.gap`.
  3. Push any non-member node whose rect intersects a hull out through its nearest
     hull edge.
  4. Re-run `remove_overlaps` (node-level), which may re-break a hull — hence the loop.
- Exit early when a pass makes no translation. Normalize the min corner to the
  origin at the end, as `layout` does today, and translate hulls by the same delta.

This is the fiddly part: keep every ordering explicit and every tie broken by index,
and assert in tests that the same input yields byte-identical output twice.

### Task 4 — plumb groups through the scene seam

`crates/waml-editor/src/scene.rs`

- Flatten `diagram.groups` (a `DiagramGroup` tree — `members: Vec<String>`,
  `children`) into `Vec<GroupSpec>` plus a parallel `Vec<(Option<String>, u8)>` of
  title+depth for emission. Member keys map through the same `index: BTreeMap<&str, usize>`
  `stress_default` already builds (`scene.rs:432`); keys absent from `sizes` are skipped.
  A group's `members` must include every descendant member so nested hulls nest.
- Change `stress_default(model_edges, sizes)` to
  `stress_default(model_edges, sizes, groups)` and call `stress::layout_grouped`.
  Note `grid_pack` is still the right fallback when `pairs.is_empty()` **and** there
  are no groups; with groups but no edges, go through `layout_grouped` so cohesion
  still applies.
- Emit `Solved.groups` from the returned hulls: `SolvedGroup { rect, shape: Shape::Shrink,
  title, depth }`. Per the decision above this renders as `GroupDraw::Skip` normally
  and dashed under show-hidden — no visible change unless that toggle is on.
- Update the two stale comments that assert the opposite: the doc comment on
  `use_stress_default` (`scene.rs:261-264`) currently claims named groups route to
  `solve_diagram`, which was never true; and `scene.rs:473-474` ("the stress layout is
  group-less").

### Task 5 — containment-aware routing on the stress path

`crates/waml-editor/src/scene.rs`, `stress_default`.

Today `route::route(&[], &rect_map, ...)` passes an empty boxes slice, so
`build_membership` yields no groups and routing degrades to bare leaf avoidance. With
hulls in hand, build the `Vec<Box>` forest (`BoxKind::Group` with `BoxId::Group(i)`
children, leaves as `BoxId::Node(key)`) and insert the hull rects into `rect_map`
under `BoxId::Group(i)` — the exact shape `geometry::solve_box` already produces
(`geometry.rs:570-578`). Then edges stop cutting through clusters.

Split from Task 4 so a routing regression can be bisected on its own.

### Task 6 — fixtures and tests

- **New fixture** `crates/waml-editor/tests/fixtures/groups-linked/` — the existing
  `groups` fixture plus relationships (`order`→`customer`, `order`→`invoice`,
  `account`→`customer`) and **no** `## Layout` section. This combination has no
  coverage anywhere in the tree today.
- `crates/waml/tests/stress_golden.rs`: golden cases for (a) two groups joined by one
  cross-group edge — members cluster, hulls disjoint; (b) a member with a strong
  outside edge — it is pulled toward the outside node, proving cohesion is soft, not
  a container; (c) nested groups — inner hull inside outer hull; (d) determinism —
  two runs byte-identical.
- `scene.rs` unit test on `groups-linked`: `scene.groups` is non-empty, every member
  rect is inside its hull, and no two sibling hulls overlap.

## Verification

```
cargo test --workspace
cd editors/vscode && npm test && npm run lint && npm run build
```

**The visual pass below is DEFERRED to a human and is NOT part of any task's
completion criteria.** Every task in this plan is complete when `cargo test
--workspace` and the vscode gate are green; no task may block on a screenshot, a
running editor, or a rendered frame. Tuning `group_len` / `group_weight` from the
render is explicit follow-up work, not part of this plan.

Then visual, on the new fixture — the only way to judge whether `group_len` /
`group_weight` defaults are tuned right:

```
pwsh scripts/run-native.ps1        # builds from $PSScriptRoot, not cwd
```

Open `groups-linked`, confirm members read as two clusters, edges are not tangled
through a foreign cluster, and toggling show-hidden draws two disjoint dashed hulls.
Then reload the existing `groups` fixture (which *does* have a `## Layout` section)
and confirm it is pixel-unchanged — it must still take the constraint path.
