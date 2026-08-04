# Performance review — full evaluation

- Dimension: Performance
- Date: 2026-08-04
- Files examined: 17 (route.rs, stress.rs, solve/mod.rs, scene.rs, class_diagram_view.rs, canvas/class/{widget.rs, render/nodes.rs, render/labels.rs, render/metrics.rs}, card/mod.rs, incremental.rs, markdown/{reparse.rs, snapshot.rs, mod.rs}, analysis.rs, uml/analysis.rs, waml-markdown-editor/widget.rs, app/event.rs, statusbar.rs, text.rs)
- Static review only; no builds or benchmarks were run.

### [P-1] Every "incremental" Markdown reparse also runs a full oracle parse in the product dialect

Severity: high
File: crates/waml-syntax/src/incremental.rs:933-962

Evidence:
```rust
    if dialect.waml_sections() {
        let oracle = crate::markdown::parser::parse_with_structure(
            new_text.clone(),
            dialect,
            new_structure.clone(),
        )?;
```
This sits at the END of the successful incremental window-reparse path in `reparse_okf_markdown_with_structure`, after the spliced tree is already built, and is used only to compare `MalformedBlock` diagnostics and island counts. `MarkdownDialect::WAML_DEFAULT` includes `WAML_SECTIONS` (crates/waml-syntax/src/text.rs:43-51), so this branch fires for every document the editor and analysis pipeline touch (`analysis.rs:1233` parses with `WAML_DEFAULT`).

Why it's wrong: the incremental machinery's whole purpose is to make an edit cost less than a full parse. For the shipping dialect, every keystroke pays: shell_map + guards + window parse + tree splice **plus** an unconditional full parse of the entire document. The incremental path is strictly more expensive than a plain full reparse; only green-node identity is saved, not time. The charter's "incremental edit cost proportional to the edit" requirement is structurally unmet.

Suggested fix: make the oracle a debug-assert/fuzz-only cross-check, or replace the two properties it verifies (malformed-block presence, island count) with checks computable from the window parse plus the already-built `new_structure` (which is derived without a full tree parse).

Confidence: CONFIRMED

### [P-2] Per-frame taffy layout: every node's card is re-measured on every draw

Severity: high
File: crates/waml-editor/src/canvas/class/render/nodes.rs:96 and crates/waml-editor/src/card/mod.rs:332-341

Evidence (`draw_card`, called from `draw_nodes` inside `ClassDiagramSurface::draw_walk` for every node, every frame):
```rust
let placed = card::measure(&card::class_shape(node, &card::mono_sheet()));
```
and `card::measure`:
```rust
pub fn measure(shape: &Shape) -> Placed {
    let mut tree: TaffyTree<LeafCtx> = TaffyTree::new();
    ...
    tree.compute_layout_with_measure(root, ...)
```
`class_shape` additionally clones `node.title` and every attribute/operation row into fresh `Shape::Text` allocations (card/mod.rs:458+), and `mono_sheet()` rebuilds the stylesheet each call.

Why it's wrong: a full taffy tree build + layout solve + O(rows) string clones per node per frame is pure per-frame waste — the card geometry depends only on the node's content and the sheet, both of which change only on scene updates. During a pan or zoom gesture (continuous redraws) a 100-node diagram runs 100 taffy solves per frame. This compounds the known 500-1200 ms zoom draw cost.

Suggested fix: cache `Placed` per node keyed on scene revision (compute it in `build_scene`, where `card_size` already runs the same measure at scene.rs:533, and store it on `SceneNode`).

Confidence: CONFIRMED

### [P-3] Edge router rebuilds the full visibility graph per edge — O(E · N^3) worst case

Severity: high
File: crates/waml/src/solve/route.rs:134-161, 320-492

Evidence (`route_keyed_with`, per edge):
```rust
    for (s, t, key, label_size) in edges {
        ...
        let mut obstacles = leaf_obstacles(rects, &[s.clone(), t.clone()]);
        obstacles.extend(group_obstacles(rects, &membership, s, t));
        obstacles.sort_by(|a, b| a.id.cmp(&b.id));
        let (ovg, srcv, tgtv) = build_ovg(&obstacles, src, tgt);
```
Inside `build_ovg`: the coordinate grid has ~2N+6 xs and ys, so O(N^2) grid points, each screened with `inflated.iter().any(|r| strictly_inside(..))` (O(N)); every `connect` runs `segment_blocked` over all N inflated rects; each attach candidate wires by scanning all vertices `for gi in 0..si` (O(N^2) per candidate, O(N) candidates). All of this is recomputed from scratch for every edge, though the obstacle set differs between edges only by the two excluded endpoints (and per-edge group membership). `place_labels_with_reroute` (solve/mod.rs:530-571) then replays the ENTIRE edge set through `route_keyed_with` up to `MAX_REROUTE_ROUNDS = 2` more times, tripling the bill when any label needs a leader.

Why it's wrong: solver cost grows superlinearly in both node and edge count. For a 100-node / 150-edge diagram the grid screening alone is on the order of 10^8-10^9 rect tests per solve, and a solve runs on every model edit that rebuilds the scene (`build_scene` → `solve_diagram_routed`, scene.rs:550). This will not stay bounded as diagrams grow; it also allocates fresh `Obstacle` vectors with cloned `BoxId`s per edge and clones `route.source`/`route.target` Strings per segment in `nudge`/`hub_spread` (route.rs:754-770).

Suggested fix: build the grid/OVG once per solve from the full obstacle set and mask the two endpoint boxes (and per-edge group obstacles) per query instead of rebuilding; index obstacles spatially (sorted intervals or a grid bucket) so segment screening is O(log N) not O(N).

Confidence: CONFIRMED (complexity by inspection; wall-clock impact not measured)

### [P-4] Incremental reparse does Θ(document) work many times over even on the incremental path

Severity: medium
File: crates/waml-syntax/src/incremental.rs:664-932, crates/waml-syntax/src/markdown/snapshot.rs:325-457

Evidence: one successful incremental keystroke walks the whole document repeatedly:
- `shell_map(&new_text)` full-text scan (incremental.rs:665) and a second `shell_map(&old)` (:753);
- `recover_exact_source(previous.root_green())` reconstructs the entire old source from the green tree (:666);
- `changes_reconstruct(&old, &new_text, changes)` re-splices the full new text (:704);
- `from_tree(previous, ...)` full old-tree projection (:769) and `from_tree(&tree, ...)` full new-tree projection (:932);
- `MarkdownReferenceMap::from_tree` full-tree walks in `change_may_affect_reference_use` (:816) and `window_reparse_may_lose_reference_resolution` (:852);
- then `snapshot.rs` runs `restore_unchanged_subtrees` (full old-tree `collect_reusable` + full candidate walk, reparse.rs:202-227) and `preserve_unchanged_island_identities` (two more full traversals, snapshot.rs:446).

Why it's wrong: each pass is linear-ish and cheap per node, but the sum means an edit costs Θ(file) — never proportional to the edit — before P-1's oracle parse is even counted. The old source is also materialized as a fresh String per keystroke (`recover_exact_source`) when the caller (`reparse_markdown`) already holds `previous.text()`.

Suggested fix: pass `previous.text()` into `reparse_okf_markdown_with_structure` instead of reconstructing it; cache `MarkdownReferenceMap` and the old `shell_map`/projection on the snapshot so guards reuse them instead of re-walking the tree per edit.

Confidence: CONFIRMED (walk structure by inspection; relative cost share not measured)

### [P-5] build_scene node-size loop is O(n^2) in nodes

Severity: medium
File: crates/waml-editor/src/scene.rs:529-535

Evidence:
```rust
    for (key, size) in &mut sizes {
        if let Some(node) = model.nodes.iter().find(|node| node.key == *key) {
            let mut projected = project_scene_node_with_display(model, node, &display);
```
A linear `find` over `model.nodes` per sized key — O(n^2) — and each hit projects the node and runs a taffy `card_size` measure. The same function builds the correct structure thirty lines later (`node_of: BTreeMap<&str, &Node>`, scene.rs:559) but only after this loop. `drawable_edges(model)` is also computed twice (:537 and :607).

Why it's wrong: `build_scene` runs on every model edit for the visible diagram; quadratic growth in node count sits on the keystroke path.

Suggested fix: hoist the `node_of` map above the sizing loop and reuse it; reuse the first `drawable_edges` result.

Confidence: CONFIRMED

### [P-6] Markdown editor rebuilds and re-translates the full-document draw-command list every frame

Severity: medium
File: crates/waml-markdown-editor/src/widget.rs:793-853

Evidence:
```rust
        let commands = build_draw_commands(&frame, &installed.plan, &installed.styles, ...)?;
        ...
        let commands = commands
            .iter()
            .map(|command| command.translated(content_origin))
            .collect::<Arc<[_]>>();
        ...
        for layer in [ ... 6 layers ... ] {
            for command in commands.iter() {
```
The layout itself is cached (`frame_layout` short-circuit in `install_layout`, widget.rs:1237-1241), but the command list is rebuilt from the whole plan, then fully copied once for translation, then iterated six times (once per layer) — every frame, including pure scroll frames.

Why it's wrong: per-frame work and allocation proportional to document size, not to what changed or what is visible. On a long document, scrolling pays O(6·C) command scans plus two O(C) allocations per frame.

Suggested fix: cache commands keyed on (revision, selection, ime, cursor-owner) and apply `content_origin` at paint time instead of materializing a translated copy; bucket commands by layer once.

Confidence: CONFIRMED

### [P-7] Per-frame HashSet of cloned Strings in draw_nodes

Severity: low
File: crates/waml-editor/src/canvas/class/render/nodes.rs:56-63

Evidence:
```rust
    let focus_keys: HashSet<String> = relations_for_visibility(...)
        .iter()
        .flat_map(|relation| [relation.subject.clone(), relation.reference.clone()])
        .collect();
```
Rebuilt with fresh String clones on every draw, though it changes only with selection/visibility state.

Why it's wrong: avoidable allocation in the per-frame path; with `HashSet<&str>` borrowed from the snapshot it would be allocation-free per entry, or it could be cached on selection change.

Suggested fix: use `HashSet<&str>` over the borrowed snapshot, or move to the selection-change path.

Confidence: CONFIRMED

### [P-8] No viewport culling: offscreen nodes are measured and drawn every frame

Severity: low
File: crates/waml-editor/src/canvas/class/render/nodes.rs:67-81

Evidence: `for node in &snapshot.scene.nodes { let screen = world_rect_to_screen(...); ... draws.node.draw_surface_abs(cx, screen); draw_card(...); }` — no check that `screen` intersects `viewport.view_rect` before issuing uniforms, quads, and the P-2 taffy measure.

Why it's wrong: when zoomed into a large diagram most nodes are offscreen but still pay full per-node draw cost each frame. Cheap to fix and multiplies the benefit of fixing P-2.

Suggested fix: skip nodes (and labels/edges similarly) whose screen rect does not intersect the view rect, with a small margin.

Confidence: CONFIRMED (absence of culling in code; frame impact not measured)

### [P-9] uml::analyze concept-to-document lookup is O(concepts × documents)

Severity: low
File: crates/waml/src/uml/analysis.rs:264-271

Evidence:
```rust
    for concept in claimed {
        let id = context.catalog.documents().iter().find_map(|(id, document)| {
            (crate::okf::id_of(document.path().as_str()) == concept.id).then_some(*id)
        })
```
Linear scan of the whole catalog per claimed concept, recomputing `id_of` per document each time.

Why it's wrong: quadratic in documents per session for bundles where most documents are claimed; runs on every analysis pass (every accepted edit).

Suggested fix: build one `BTreeMap<ConceptId, DocumentId>` before the loop.

Confidence: CONFIRMED

## Not findings

- **Zoom text re-rasterization**: mitigated — a fixed raster ladder exists (`FONT_RASTER_SIZES`/`font_raster_size`, canvas/primitives.rs:4-8) and is used by class nodes, groups, edge labels, and the behavior canvas (`behavior/render/mod.rs:45-47` compensates via `font_scale`), so continuous zoom does not mint new raster sizes.
- **Edge label legibility cutoff**: labels are skipped entirely below `MIN_LEGIBLE_PX` (labels.rs:24, 51), bounding zoomed-out label cost.
- **Markdown layout caching**: `install_layout` returns the cached `frame_layout` when nothing is pending (widget.rs:1237-1241); width changes correctly force reflow via bit-exact width compare.
- **Solve is on-change, not per-frame**: `build_scene`/`solve_diagram_routed` run from `sync`/`update_scene` (class_diagram_view.rs:323, 420), never from `draw_walk`.
- **Stress layout is bounded**: SMACOF capped at `max_iter: 300` with epsilon convergence, per connected component, deterministic (stress.rs:19-39).
- **A\* label-pressure caching**: `band_blocked_fraction` is cached per (v, w) pair inside a search (route.rs:660-694).
- **Nudge/hub-spread passes**: channel-grouped sweeps, O(S log S) in segments — no quadratic pair scan (route.rs:739-803).
- **single_text_change**: proper prefix/suffix diff, not a whole-file replacement, so downstream ChangeMaps stay minimal (analysis.rs:1370-1405).
- **UML island reuse**: prior island syntax trees are reused by owner/range identity across edits (uml/analysis.rs:324-337).
- **Analysis snapshot reuse**: unchanged documents are reused by `Arc::ptr_eq` on the text (analysis.rs:1153), so a one-document edit reparses one document.
- **Redraw scoping**: state setters gate on change before `redraw` (e.g. `set_projection_stale`, widget.rs:800-805); interaction effects carry an explicit `redraw` flag.
- **Web boot path**: mitigations are landed and instrumented — batched shader linking, font pruning (`scripts/prune-web-fonts.mjs`), and measurement probes (`scripts/measure-web-boot.mjs`, `attribute-web-boot.mjs`); nothing in this review adds to the boot path. Not re-measured (static review).
- **Statusbar per-draw String**: one small format per draw of a 24px strip; negligible and only on redraw.
- **fps meter**: per-event but O(1) sampling (app/event.rs:13-28).
