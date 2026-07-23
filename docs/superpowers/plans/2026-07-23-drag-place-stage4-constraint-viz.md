# Drag-to-place Stage 4 — persistent constraint visibility Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make placement constraints visible at rest — always-on relation glyphs, a red conflict state-color driven by a leave-one-out attribution pass, and debug-grade group bounds — so authoring a diagram (and telling a solver bug from a genuine contradiction) stops being guesswork.

**Architecture:** One unit-testable scene-layer change carries the load-bearing logic (a `SceneRelation.conflicting` flag set by a best-effort leave-one-out re-solve in `build_scene`). The rest is native-canvas draw work: extract the relation connector+glyph draw currently inlined in the armed-drag overlay into one shared helper, then invoke it always-on from `draw_walk` (red when `conflicting`) and strengthen the existing group-rect draw into a debug outline. No `waml`, `waml-ops-dto`, web, or wasm changes.

**Tech Stack:** Rust, makepad (native canvas), `cargo test --workspace`.

## Global Constraints

- Native-canvas only. NO web / pnpm / wasm work in this change (spec says so explicitly). Do not add pnpm steps.
- The gate for EVERY task is `cargo test --workspace` (run from the worktree root). It must be green before you commit.
- Warnings-as-errors + exhaustive struct literals are in force: adding a field to `SceneRelation` breaks every struct-literal construction site until each names the new field. Task 1 fixes all of them in the same commit or the gate fails.
- This is a **playable-tonight v1**. The direction-glyph is a deliberate PLACEHOLDER (`dir_word` text), NOT a gap — do not build final glyph art. Group bounds are debug-grade, not final chrome.
- Non-goals are BINDING — do NOT add: hover-trace, conflict-ring/transitive-cycle attribution, override-amber affordance, group-scoped drag, final glyph art, or a viz on/off toggle.
- Tasks 2–4 are canvas draw work with **no screenshot-drivable or pixel-level unit assertion** — their gate is compile + the existing unit suite staying green. Final visual sign-off for those is the user running `scripts/run-native.ps1 -Optimized mini` and looking. State this in the commit; do not claim a draw task is "verified" beyond compile+green.

---

### Task 1: `SceneRelation.conflicting` field + leave-one-out attribution pass

**Files:**
- Modify: `crates/waml-editor/src/scene.rs` (struct `SceneRelation` ~:86; `project_relations` push ~:208; `build_scene` relation construction ~:401; add two private fns)
- Modify: `crates/waml-editor/src/canvas.rs` (test-only `SceneRelation` literals ~:1700, ~:1705, ~:1710 — add the new field so the gate compiles)
- Test: `crates/waml-editor/src/scene.rs` (`#[cfg(test)] mod tests`, alongside `oracle_flags_a_contradictory_placement` ~:593)

**Interfaces:**
- Produces: `SceneRelation` gains `pub conflicting: bool` (default `false`). Set by a new private `fn attribute_conflicts(model: &Model, diagram: &Diagram, expanded: &HashSet<String>, diags: &[Diagnostic], relations: &mut [SceneRelation])`, backed by a new private `fn solve_diags(model: &Model, diagram: &Diagram, expanded: &HashSet<String>) -> Vec<Diagnostic>`. `build_scene`'s signature is unchanged; only the `Scene.relations` it returns now carry attribution.
- Consumes: existing `project_relations`, `placement_is_pair` (~:474), `use_stress_default`, `drawable_edges`, `solve_diagram`, `DiagCode::LayoutConflict`.

**Why a separate `solve_diags` (do NOT re-enter `build_scene`):** the attribution pass re-solves scratch clones. If it called `build_scene`, `build_scene` → `attribute_conflicts` → `build_scene` would recurse (a still-conflicting scratch would launch its own O(R) attribution each level → blow-up). `solve_diags` runs sizing + `solve_diagram` and returns diagnostics ONLY (no scene, no attribution), so it is recursion-free. Its `LayoutConflict` count is directly comparable to `build_scene`'s own `diags` (same `solve_diagram` source).

- [ ] **Step 1: Write the failing tests**

Add to the `#[cfg(test)] mod tests` block in `crates/waml-editor/src/scene.rs`:

```rust
#[test]
fn attribution_marks_the_culprits_of_a_contradiction() {
    use waml::syntax::{Direction, LayoutStatement, NameRef, Operand, OperandRef};
    // mini already authors `Order left of Customer`. Add the reversed pair
    // `Customer left of Order` (a DIFFERENT ordered pair, so neither replaces
    // the other) — both coexist, the solver cannot satisfy them and emits a
    // LayoutConflict. Leave-one-out: removing EITHER culprit resolves it, so
    // both are marked conflicting; `payment-gateway below order` is independent
    // and stays false.
    let model = mini();
    let mut diagram = model.diagrams[0].clone();
    let link = |slug: &str| Operand {
        ref_: OperandRef::Name(NameRef::Link {
            title: title_for(&model, slug),
            slug: slug.to_string(),
        }),
        axis: None,
        hints: Vec::new(),
    };
    diagram.layout.push(LayoutStatement::Placement {
        operands: vec![link("customer"), link("order")],
        directions: vec![Direction::LeftOf],
    });

    let (scene, diags) = build_scene(&model, &diagram, &std::collections::HashSet::new());
    use waml::diagnostic::DiagCode;
    assert!(
        diags.iter().any(|d| d.code == DiagCode::LayoutConflict),
        "fixture must be genuinely contradictory: {diags:?}"
    );

    let conflicting = |subj: &str, refr: &str| {
        scene
            .relations
            .iter()
            .find(|r| r.subject == subj && r.reference == refr)
            .unwrap_or_else(|| panic!("relation {subj} -> {refr} missing: {:?}", scene.relations))
            .conflicting
    };
    assert!(conflicting("order", "customer"), "order->customer is a culprit");
    assert!(conflicting("customer", "order"), "customer->order is a culprit");
    assert!(
        !conflicting("payment-gateway", "order"),
        "independent relation must NOT be marked conflicting"
    );
}

#[test]
fn attribution_marks_nothing_on_a_clean_diagram() {
    // mini's default layout is satisfiable (no LayoutConflict), so the common
    // path must leave every relation conflicting == false and do no extra work.
    let model = mini();
    let (scene, diags) = build_scene(&model, &model.diagrams[0], &std::collections::HashSet::new());
    use waml::diagnostic::DiagCode;
    assert!(
        !diags.iter().any(|d| d.code == DiagCode::LayoutConflict),
        "mini must be conflict-free: {diags:?}"
    );
    assert!(
        scene.relations.iter().all(|r| !r.conflicting),
        "clean diagram must mark no relation conflicting: {:?}",
        scene.relations
    );
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p waml-editor attribution`
Expected: FAIL — `no field 'conflicting' on type '&SceneRelation'` (field does not exist yet). Compile error is the expected "fail" here.

- [ ] **Step 3: Add the `conflicting` field to `SceneRelation`**

In `crates/waml-editor/src/scene.rs`, the struct at ~:86 — add the field:

```rust
#[derive(Debug, Clone, PartialEq)]
pub struct SceneRelation {
    pub subject: String,
    pub reference: String,
    pub dir: waml::syntax::Direction,
    /// Best-effort leave-one-out conflict attribution, set by `build_scene`:
    /// `true` iff removing just this relation reduces the solver's
    /// `LayoutConflict` count (i.e. it participates in a contradiction).
    /// Defaults `false`; only ever `true` on an already-conflicted diagram.
    pub conflicting: bool,
}
```

- [ ] **Step 4: Name the new field at every construction site**

In `crates/waml-editor/src/scene.rs`, `project_relations` push (~:208):

```rust
                    out.push(SceneRelation {
                        subject: subject.to_string(),
                        reference: reference.to_string(),
                        dir: directions[0],
                        conflicting: false,
                    });
```

In `crates/waml-editor/src/canvas.rs`, the three test literals in `relations_in_scope_keeps_only_relations_touching_dragged_or_target` (~:1700, ~:1705, ~:1710) — add `conflicting: false,` to each:

```rust
            SceneRelation {
                subject: "order".into(),
                reference: "customer".into(),
                dir: Direction::LeftOf,
                conflicting: false,
            },
            SceneRelation {
                subject: "payment-gateway".into(),
                reference: "order".into(),
                dir: Direction::Below,
                conflicting: false,
            },
            SceneRelation {
                subject: "invoice".into(),
                reference: "shipment".into(),
                dir: Direction::Above,
                conflicting: false,
            },
```

- [ ] **Step 5: Add `solve_diags` and `attribute_conflicts`**

In `crates/waml-editor/src/scene.rs`, add these two private fns (place them next to `placement_would_conflict` ~:495, which they mirror):

```rust
/// Run the solver for `diagram` and return only its diagnostics — no scene
/// projection, no conflict attribution. `attribute_conflicts` re-solves scratch
/// clones through this so it never re-enters `build_scene` (which would recurse
/// through attribution). The `LayoutConflict` count here is directly comparable
/// to `build_scene`'s own `diags` (same `solve_diagram` source).
fn solve_diags(
    model: &Model,
    diagram: &Diagram,
    expanded: &std::collections::HashSet<String>,
) -> Vec<Diagnostic> {
    let sizes = crate::sizing::size_map(model, diagram, expanded);
    let edges: Vec<(BoxId, BoxId)> = drawable_edges(model)
        .into_iter()
        .map(|e| (BoxId::Node(e.source.clone()), BoxId::Node(e.target.clone())))
        .collect();
    if use_stress_default(diagram) {
        Vec::new()
    } else {
        solve_diagram(diagram, &edges, &sizes, &SolveConfig::default()).1
    }
}

/// Best-effort leave-one-out conflict attribution. Runs ONLY when the solve
/// already emitted a `LayoutConflict`; the clean path (the common case) returns
/// immediately with every relation left `false`. When conflicted, for each
/// projected relation it drops just that ordered placement from a scratch clone,
/// re-solves via `solve_diags`, and marks `conflicting` iff the `LayoutConflict`
/// count drops (the relation participates in a contradiction). O(relations)
/// re-solves, fired at scene-build time only, and only on an already-conflicted
/// diagram — never per frame.
fn attribute_conflicts(
    model: &Model,
    diagram: &Diagram,
    expanded: &std::collections::HashSet<String>,
    diags: &[Diagnostic],
    relations: &mut [SceneRelation],
) {
    use waml::diagnostic::DiagCode;
    let base = diags
        .iter()
        .filter(|d| d.code == DiagCode::LayoutConflict)
        .count();
    if base == 0 {
        return; // common path: satisfiable, everything stays false
    }
    for rel in relations.iter_mut() {
        let mut scratch = diagram.clone();
        scratch
            .layout
            .retain(|s| !placement_is_pair(s, &rel.subject, &rel.reference));
        let after = solve_diags(model, &scratch, expanded)
            .iter()
            .filter(|d| d.code == DiagCode::LayoutConflict)
            .count();
        rel.conflicting = after < base;
    }
}
```

- [ ] **Step 6: Wire the attribution pass into `build_scene`**

In `crates/waml-editor/src/scene.rs`, `build_scene` returns the `Scene` at ~:396. Replace the inline `relations: project_relations(diagram),` in the `Scene` literal with a pre-built, attributed vec. Just before the `Scene { ... }` return (after the `edges` loop, ~:395), insert:

```rust
    let mut relations = project_relations(diagram);
    attribute_conflicts(model, diagram, expanded, &diags, &mut relations);
```

and change the `Scene` literal's relations line to:

```rust
            relations,
```

- [ ] **Step 7: Run the tests to verify they pass**

Run: `cargo test -p waml-editor attribution`
Expected: PASS — both `attribution_marks_the_culprits_of_a_contradiction` and `attribution_marks_nothing_on_a_clean_diagram`.

- [ ] **Step 8: Run the full gate**

Run: `cargo test --workspace`
Expected: PASS — the whole workspace, including the canvas.rs literal fixes and the untouched scene tests.

- [ ] **Step 9: Commit**

```bash
git add crates/waml-editor/src/scene.rs crates/waml-editor/src/canvas.rs
git commit -m "feat(scene): SceneRelation.conflicting + leave-one-out attribution"
```

---

### Task 2: Extract the shared relation connector+glyph draw helper

**Files:**
- Modify: `crates/waml-editor/src/canvas.rs` (`draw_drag_overlay` ~:1292, its inlined `legs` loop ~:1330-1351; add a new method on `impl GraphCanvas`)

**Interfaces:**
- Consumes: `node_screen_center` (~:1283), `fill_rect` (~:1494), `dir_word` (~:489), `draw_mono_dim` pen (field ~:232).
- Produces: `fn draw_relation_connector(&mut self, cx: &mut Cx2d, subject_idx: usize, reference_idx: usize, dir: waml::syntax::Direction, color: Vec4)` — draws one relation as an orthogonal-L connector between the two node centers plus a placeholder direction glyph at the elbow. Task 3 reuses it.

**No unit assertion:** this is draw code; no test inspects pixels. The gate is compile + the existing suite (`relations_in_scope_*` etc.) staying green. Behavior note: the armed-drag overlay now also stamps the placeholder glyph at each relation's elbow (previously it drew only the two connector bars). That is the intended unification onto one draw path, not a regression; no test asserts the old glyph-less output.

- [ ] **Step 1: Add the shared helper**

In `crates/waml-editor/src/canvas.rs`, inside `impl GraphCanvas` (next to `draw_drag_overlay`), add:

```rust
    /// Draw one placement relation as an orthogonal L connector between the
    /// reference node (b) and the subject node (a) centers — horizontal leg then
    /// vertical leg — plus a PLACEHOLDER direction glyph at the elbow. `color`
    /// carries the weight/tint: calm slate for the persistent overlay, red for a
    /// `conflicting` relation, brighter slate for the armed-drag emphasis. The
    /// glyph is `dir_word` text standing in for final art (out of scope for v1).
    /// Shared by the always-on overlay (`draw_relations_overlay`) and the
    /// armed-drag overlay (`draw_drag_overlay`) so they never diverge.
    fn draw_relation_connector(
        &mut self,
        cx: &mut Cx2d,
        subject_idx: usize,
        reference_idx: usize,
        dir: waml::syntax::Direction,
        color: Vec4,
    ) {
        let a = self.node_screen_center(subject_idx);
        let b = self.node_screen_center(reference_idx);
        self.fill_rect(cx, a.x.min(b.x), b.y, (a.x - b.x).abs(), 2.0, color);
        self.fill_rect(cx, a.x, a.y.min(b.y), 2.0, (a.y - b.y).abs(), color);
        // Placeholder direction glyph at the elbow corner (a.x, b.y).
        self.draw_mono_dim.text_style.font_size = 11.0;
        self.draw_mono_dim
            .draw_abs(cx, dvec2(a.x + 4.0, b.y - 6.0), dir_word(dir));
    }
```

- [ ] **Step 2: Rewire `draw_drag_overlay` to use the helper**

In `draw_drag_overlay` (~:1330), the `legs` collection currently drops the direction. Capture `rel.dir`, and replace the two-`fill_rect` loop body with a call. Change:

```rust
            let legs: Vec<(usize, usize)> =
                relations_in_scope(&self.scene.relations, &a_key, &target_key)
                    .into_iter()
                    .filter_map(|rel| {
                        let si = self.scene.nodes.iter().position(|n| n.key == rel.subject)?;
                        let ri = self
                            .scene
                            .nodes
                            .iter()
                            .position(|n| n.key == rel.reference)?;
                        Some((si, ri))
                    })
                    .collect();
            for (si, ri) in legs {
                let a = self.node_screen_center(si);
                let b = self.node_screen_center(ri);
                // Orthogonal L from reference (b) to subject (a): horizontal
                // leg then vertical leg, 2px slate.
                let ind = vec4(0.55, 0.62, 0.72, 0.7);
                self.fill_rect(cx, a.x.min(b.x), b.y, (a.x - b.x).abs(), 2.0, ind);
                self.fill_rect(cx, a.x, a.y.min(b.y), 2.0, (a.y - b.y).abs(), ind);
            }
```

to:

```rust
            let legs: Vec<(usize, usize, waml::syntax::Direction)> =
                relations_in_scope(&self.scene.relations, &a_key, &target_key)
                    .into_iter()
                    .filter_map(|rel| {
                        let si = self.scene.nodes.iter().position(|n| n.key == rel.subject)?;
                        let ri = self
                            .scene
                            .nodes
                            .iter()
                            .position(|n| n.key == rel.reference)?;
                        Some((si, ri, rel.dir))
                    })
                    .collect();
            for (si, ri, dir) in legs {
                // Armed-drag emphasis: brighter slate than the persistent overlay.
                let ind = vec4(0.55, 0.62, 0.72, 0.7);
                self.draw_relation_connector(cx, si, ri, dir, ind);
            }
```

- [ ] **Step 3: Run the gate**

Run: `cargo test --workspace`
Expected: PASS — pure extraction; `relations_in_scope_*` and every other test stay green.

- [ ] **Step 4: Commit**

```bash
git add crates/waml-editor/src/canvas.rs
git commit -m "refactor(canvas): extract shared relation connector+glyph draw helper"
```

---

### Task 3: Always-on persistent relation overlay + red conflict tint

**Files:**
- Modify: `crates/waml-editor/src/canvas.rs` (`draw_walk` — the overlay call site ~:1261-1264; add a new method on `impl GraphCanvas`)

**Interfaces:**
- Consumes: `draw_relation_connector` (Task 2), `Scene.relations` with `conflicting` (Task 1).
- Produces: `fn draw_relations_overlay(&mut self, cx: &mut Cx2d)` — draws the FULL `self.scene.relations` set at a calm weight, red when `conflicting`, independent of drag state.

**Borrow note:** resolve each relation to owned `(subject_idx, reference_idx, dir, conflicting)` tuples FIRST so the immutable borrow of `self.scene.relations` ends before `draw_relation_connector` (a `&mut self` method) runs — mirrors the existing `legs` pattern in `draw_drag_overlay`.

**No unit assertion:** draw code; gate is compile + green suite. Interactive sign-off only (see Step 4).

- [ ] **Step 1: Add the persistent-overlay method**

In `crates/waml-editor/src/canvas.rs`, inside `impl GraphCanvas`, add:

```rust
    /// Always-on relation overlay: every projected placement relation drawn at a
    /// calm weight so the diagram's authored structure is legible at rest (not
    /// only mid-drag). A relation the attribution pass flagged `conflicting`
    /// paints red — the bug-vs-contradiction signal. Independent of drag state;
    /// the armed-drag overlay (`draw_drag_overlay`) still layers its scoped,
    /// brighter emphasis on top.
    fn draw_relations_overlay(&mut self, cx: &mut Cx2d) {
        // Own the tuples before drawing: `fill_rect`/`draw_relation_connector`
        // are `&mut self`, so the immutable borrow of `relations` must end first.
        let legs: Vec<(usize, usize, waml::syntax::Direction, bool)> = self
            .scene
            .relations
            .iter()
            .filter_map(|rel| {
                let si = self.scene.nodes.iter().position(|n| n.key == rel.subject)?;
                let ri = self.scene.nodes.iter().position(|n| n.key == rel.reference)?;
                Some((si, ri, rel.dir, rel.conflicting))
            })
            .collect();
        for (si, ri, dir, conflicting) in legs {
            let color = if conflicting {
                vec4(0.80, 0.22, 0.22, 0.85) // red culprit
            } else {
                vec4(0.55, 0.62, 0.72, 0.45) // calm slate
            };
            self.draw_relation_connector(cx, si, ri, dir, color);
        }
    }
```

- [ ] **Step 2: Invoke it from `draw_walk`, before the armed-drag overlay**

In `draw_walk` (~:1261), change:

```rust
        // SPIKE (drag-place): live placement overlay on top of everything.
        if self.drag_moved {
            self.draw_drag_overlay(cx, rect);
        }
```

to:

```rust
        // Persistent relation overlay: the full projected relation set, always-on
        // at a calm weight (red where `conflicting`), so authored placement is
        // visible at rest. Drawn under the armed-drag overlay's scoped emphasis.
        self.draw_relations_overlay(cx);

        // SPIKE (drag-place): live placement overlay on top of everything.
        if self.drag_moved {
            self.draw_drag_overlay(cx, rect);
        }
```

- [ ] **Step 3: Run the gate**

Run: `cargo test --workspace`
Expected: PASS — no test asserts overlay pixels; compile is the meaningful check.

- [ ] **Step 4: Interactive sign-off (not a gate, note in commit)**

Run: `scripts/run-native.ps1 -Optimized mini`
Expected (visual, user-driven): at rest (no drag) the `order left of customer` and `payment-gateway below order` relations show a thin connector + `left of` / `below` placeholder glyph between the related node pairs. Final sign-off is the user looking; do not claim more than compile+green.

- [ ] **Step 5: Commit**

```bash
git add crates/waml-editor/src/canvas.rs
git commit -m "feat(canvas): always-on persistent relation overlay + red conflict tint"
```

---

### Task 4: Debug-grade group bounds (outline + title)

**Files:**
- Modify: `crates/waml-editor/src/canvas.rs` (`draw_walk` group loop ~:1076-1094)

**Interfaces:**
- Consumes: `SolvedGroup { rect, title, depth }` (already on `self.scene.groups`), `fill_rect`, `draw_group`/`draw_text` pens, `camera.world_to_local`.
- Produces: no new public surface — strengthens the existing group draw in place.

**Borrow note:** the current loop is `for group in &self.scene.groups { ... }` and gets away with `self.draw_group.draw_abs` / `self.draw_text.draw_abs` (disjoint field borrows). Adding the outline needs `fill_rect` (`&mut self`), which cannot run inside that immutable borrow — so collect the per-group screen data into an owned `Vec` first, then draw.

**No unit assertion:** draw code; gate is compile + green suite. Interactive sign-off only.

- [ ] **Step 1: Replace the group loop with collect-first + debug outline**

In `crates/waml-editor/src/canvas.rs`, replace the existing group block (~:1074-1094):

```rust
        // Groups: framed rects behind everything else. Deeper nesting is drawn
        // with the same fill; draw-order (shallow first) leaves inner groups on top.
        for group in &self.scene.groups {
            let (lx, ly) = self.camera.world_to_local(group.rect.x, group.rect.y);
            let screen = Rect {
                pos: dvec2(rect.pos.x + lx, rect.pos.y + ly),
                size: dvec2(
                    group.rect.w * self.camera.zoom,
                    group.rect.h * self.camera.zoom,
                ),
            };
            self.draw_group.draw_abs(cx, screen);
            if let Some(title) = &group.title {
                self.draw_text.text_style.font_size = (12.0 * zoom) as f32;
                self.draw_text.draw_abs(
                    cx,
                    dvec2(screen.pos.x + 6.0 * zoom, screen.pos.y + 4.0 * zoom),
                    title,
                );
            }
        }
```

with:

```rust
        // Groups: framed rects behind everything else, now with a debug-grade
        // outline so group extents are legible while organizing. Deeper nesting
        // keeps the same fill; draw-order (shallow first) leaves inner groups on
        // top. Collect screen rects first so `fill_rect` (&mut self) can stroke
        // the outline without holding the `self.scene.groups` borrow.
        let group_draws: Vec<(Rect, Option<String>)> = self
            .scene
            .groups
            .iter()
            .map(|g| {
                let (lx, ly) = self.camera.world_to_local(g.rect.x, g.rect.y);
                let screen = Rect {
                    pos: dvec2(rect.pos.x + lx, rect.pos.y + ly),
                    size: dvec2(g.rect.w * self.camera.zoom, g.rect.h * self.camera.zoom),
                };
                (screen, g.title.clone())
            })
            .collect();
        for (screen, title) in group_draws {
            self.draw_group.draw_abs(cx, screen);
            // Debug outline: four thin slate bars hugging the rect border.
            let ol = vec4(0.45, 0.52, 0.60, 0.85);
            let t = 1.5;
            self.fill_rect(cx, screen.pos.x, screen.pos.y, screen.size.x, t, ol);
            self.fill_rect(
                cx,
                screen.pos.x,
                screen.pos.y + screen.size.y - t,
                screen.size.x,
                t,
                ol,
            );
            self.fill_rect(cx, screen.pos.x, screen.pos.y, t, screen.size.y, ol);
            self.fill_rect(
                cx,
                screen.pos.x + screen.size.x - t,
                screen.pos.y,
                t,
                screen.size.y,
                ol,
            );
            if let Some(title) = &title {
                self.draw_text.text_style.font_size = (12.0 * zoom) as f32;
                self.draw_text.draw_abs(
                    cx,
                    dvec2(screen.pos.x + 6.0 * zoom, screen.pos.y + 4.0 * zoom),
                    &title,
                );
            }
        }
```

- [ ] **Step 2: Run the gate**

Run: `cargo test --workspace`
Expected: PASS — draw-only change; suite stays green.

- [ ] **Step 3: Interactive sign-off (not a gate, note in commit)**

Run: `scripts/run-native.ps1 -Optimized mini` (and, if convenient, a fixture with a `### Group` heading).
Expected (visual): each group's bounding rect shows a visible outline + its title label at rest.

- [ ] **Step 4: Commit**

```bash
git add crates/waml-editor/src/canvas.rs
git commit -m "feat(canvas): debug-grade group bounds outline + title"
```

---

## Self-Review

**Spec coverage:**
- Feature 1 (persistent relation glyphs) → Task 2 (shared connector+glyph helper) + Task 3 (always-on invocation). ✔
- Feature 2 (conflict state-color): attribution → Task 1 (unit-tested `conflicting` + leave-one-out pass); red draw → Task 3 (red tint on `conflicting`). ✔
- Feature 3 (group bounds viz) → Task 4 (debug outline + title). ✔
- Non-goals (hover-trace, conflict-ring, override-amber, group-scoped drag, final glyph art, viz toggle) → none planned. ✔
- Testing: scene attribution is the one cleanly-TDD'd task (Task 1); draw tasks are compile+green + interactive sign-off, stated explicitly. ✔
- `depth` tint was optional in the spec ("if cheap"); omitted to keep the group fill's existing `#[live]` styling intact — a deliberate scope call, not a gap.

**Placeholder scan:** no TBD/TODO/"handle edge cases"/"write tests for the above" — every code step carries full code; the direction glyph is an explicit intended placeholder, not a plan placeholder.

**Type consistency:** `conflicting: bool` named identically at struct def + `project_relations` push + 3 canvas test literals. `draw_relation_connector(cx, subject_idx, reference_idx, dir, color)` signature matches its two call sites (Task 2 drag overlay, Task 3 persistent overlay). `solve_diags` / `attribute_conflicts` signatures match their single call site in `build_scene`. `waml::syntax::Direction` used consistently.
