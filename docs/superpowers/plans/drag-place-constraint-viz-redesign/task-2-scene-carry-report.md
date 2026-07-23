### Task 2: Delete Stage-4 attribution; carry the solver report into the scene

**Files:**
- Modify: `crates/waml-editor/src/scene.rs` (drop `SceneRelation.conflicting`; delete `attribute_conflicts`; add `SceneConflict` + `Scene.conflicts`; build it from `solve_diagram_reported`; replace the two attribution tests)
- Modify: `crates/waml-editor/src/canvas.rs` (stop reading the removed `conflicting` field so the crate still compiles; drop `conflicting` from the one test literal)

**Interfaces:**
- Consumes (from Task 1): `waml::solve::{solve_diagram_reported, DroppedPlacement}`; `waml::solve::Constraint`/`BoxId`/`Direction`.
- Produces (consumed by Tasks 4/5/7):
  - `crate::scene::SceneRelation { pub subject: String, pub reference: String, pub dir: waml::syntax::Direction }` (the `conflicting` field is GONE).
  - `crate::scene::SceneConflict { pub dropped: SceneRelation, pub conflicts_with: Vec<SceneRelation> }`
  - `crate::scene::Scene { nodes, groups, edges, relations, conflicts: Vec<SceneConflict> }` (new field; `#[derive(Default)]` still holds).
- Unchanged: `placement_would_conflict` and its helper `solve_diags` STAY (they power the drag-time compass conflict-zone reddening — a different, still-valid use; only the Stage-4 *attribution* role is removed). `project_relations`' pair projection is unchanged except for the dropped struct field.

**Removal is user-approved** (spec §"Data / Type Changes": remove `SceneRelation.conflicting`, `attribute_conflicts`, the leave-one-out `solve_diags`-based attribution in `build_scene`).

---

- [ ] **Step 1: Write the failing scene tests (and delete the two attribution tests)**

In `crates/waml-editor/src/scene.rs`, DELETE these two now-obsolete tests entirely:
- `attribution_marks_the_culprits_of_a_contradiction`
- `attribution_marks_nothing_on_a_clean_diagram`

Add these replacements inside the same `#[cfg(test)] mod tests`:

```rust
#[test]
fn clean_diagram_has_no_conflicts() {
    // mini's default layout is satisfiable, so the solver drops nothing and the
    // scene carries an empty conflict report.
    let model = mini();
    let (scene, diags) = build_scene(&model, &model.diagrams[0], &std::collections::HashSet::new());
    use waml::diagnostic::DiagCode;
    assert!(!diags.iter().any(|d| d.code == DiagCode::LayoutConflict), "mini must be conflict-free: {diags:?}");
    assert!(scene.conflicts.is_empty(), "clean diagram must report no conflicts: {:?}", scene.conflicts);
}

#[test]
fn contradiction_surfaces_in_scene_conflicts() {
    use waml::syntax::{Direction, LayoutStatement, NameRef, Operand, OperandRef};
    // mini authors `Order left of Customer`. Add the reversed pair
    // `Customer left of Order`: a different ordered pair, so both coexist and the
    // solver cannot satisfy them. The dropped placement + its contradiction set
    // surface in scene.conflicts (NO canvas red, NO leave-one-out).
    let model = mini();
    let mut diagram = model.diagrams[0].clone();
    let link = |slug: &str| Operand {
        ref_: OperandRef::Name(NameRef::Link { title: title_for(&model, slug), slug: slug.to_string() }),
        axis: None,
        hints: Vec::new(),
    };
    diagram.layout.push(LayoutStatement::Placement {
        operands: vec![link("customer"), link("order")],
        directions: vec![Direction::LeftOf],
    });

    let (scene, diags) = build_scene(&model, &diagram, &std::collections::HashSet::new());
    use waml::diagnostic::DiagCode;
    assert!(diags.iter().any(|d| d.code == DiagCode::LayoutConflict), "must be genuinely contradictory: {diags:?}");
    assert!(!scene.conflicts.is_empty(), "contradiction must surface in scene.conflicts");
    // Every reported conflict names a real projected relation and a non-empty
    // contradiction set; the independent `payment-gateway below order` never appears.
    for c in &scene.conflicts {
        assert!(!c.conflicts_with.is_empty(), "a dropped relation must list what it conflicts with");
        let touches = |r: &SceneRelation| (r.subject == "order" && r.reference == "customer")
            || (r.subject == "customer" && r.reference == "order");
        assert!(touches(&c.dropped), "dropped relation should be one of the reversed pair: {:?}", c.dropped);
        assert!(c.conflicts_with.iter().any(touches), "conflict set should include the opposing placement");
        assert!(
            !(c.dropped.subject == "payment-gateway"),
            "the independent placement must not be reported as dropped"
        );
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p waml-editor --lib scene`
Expected: FAIL to compile — `Scene` has no `conflicts` field, `SceneConflict` is undefined.

- [ ] **Step 3: Drop `conflicting` from `SceneRelation`; add `SceneConflict`; add `Scene.conflicts`**

In `scene.rs`, replace the `SceneRelation` struct (remove the `conflicting` field and its doc comment):

```rust
/// A placement relation projected from the diagram's `## Layout`: a 2-operand
/// single-direction placement, operands resolved to `SceneNode.key` slugs.
/// Multi-operand / alignment statements are not projected.
#[derive(Debug, Clone, PartialEq)]
pub struct SceneRelation {
    pub subject: String,
    pub reference: String,
    pub dir: waml::syntax::Direction,
}

/// A placement the solver could not honor, projected from `DroppedPlacement`
/// into slug-level relations for the editor's conflict error list.
#[derive(Debug, Clone, PartialEq)]
pub struct SceneConflict {
    pub dropped: SceneRelation,
    pub conflicts_with: Vec<SceneRelation>,
}
```

Add the `conflicts` field to `Scene`:

```rust
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Scene {
    pub nodes: Vec<SceneNode>,
    pub groups: Vec<SolvedGroup>,
    pub edges: Vec<SceneEdge>,
    pub relations: Vec<SceneRelation>,
    pub conflicts: Vec<SceneConflict>,
}
```

In `project_relations`, remove the `conflicting: false,` line from the `SceneRelation { ... }` it pushes.

- [ ] **Step 4: Add the `DroppedPlacement` → `SceneConflict` projector**

Add this helper near `project_relations` in `scene.rs`:

```rust
/// Project a single solver `Constraint::Place` into a slug-level `SceneRelation`.
/// Non-`Place` or non-`Node` operands (group/inline endpoints) yield `None`, so
/// only 2-node placements — the ones the conflict list can name — survive.
fn relation_of_constraint(c: &waml::solve::Constraint) -> Option<SceneRelation> {
    use waml::solve::{BoxId, Constraint};
    if let Constraint::Place { a: BoxId::Node(subject), b: BoxId::Node(reference), dir } = c {
        Some(SceneRelation { subject: subject.clone(), reference: reference.clone(), dir: *dir })
    } else {
        None
    }
}

/// Project the solver's dropped-placement report into `SceneConflict`s. A dropped
/// placement whose subject/reference don't both resolve to node slugs is skipped
/// (it can't be named in the DSL error list).
fn project_conflicts(dropped: &[waml::solve::DroppedPlacement]) -> Vec<SceneConflict> {
    dropped
        .iter()
        .filter_map(|d| {
            let dropped = relation_of_constraint(&d.relation)?;
            let conflicts_with = d.conflicts_with.iter().filter_map(relation_of_constraint).collect();
            Some(SceneConflict { dropped, conflicts_with })
        })
        .collect()
}
```

- [ ] **Step 5: Rewrite `build_scene`'s solve + relation tail**

In `build_scene`, change the solve call so the non-stress branch uses the reported entry point, and capture the report. Replace the `let (solved, diags) = if use_stress_default(diagram) { ... } else { ... };` block with:

```rust
    let (solved, diags, dropped) = if use_stress_default(diagram) {
        (stress_default(model, &sizes), Vec::new(), Vec::new())
    } else {
        waml::solve::solve_diagram_reported(diagram, &edges, &sizes, &SolveConfig::default())
    };
```

At the end of `build_scene`, replace the relation-attribution tail:

```rust
    let mut relations = project_relations(diagram);
    attribute_conflicts(model, diagram, expanded, &diags, &mut relations);
```

with:

```rust
    let relations = project_relations(diagram);
    let conflicts = project_conflicts(&dropped);
```

and add `conflicts` to the returned `Scene { ... }`:

```rust
    (
        Scene { nodes, groups: solved.groups.clone(), edges, relations, conflicts },
        diags,
    )
```

Update the `waml::solve::{...}` import at the top of `scene.rs` if `solve_diagram_reported`/`DroppedPlacement`/`Constraint`/`BoxId` are not already in scope. Simplest: reference them fully-qualified as written above (no import edit needed) — `solve_diagram` in the existing import list can stay (still used elsewhere) or be removed if now unused (clippy will flag an unused import; drop it if so).

- [ ] **Step 6: Delete `attribute_conflicts`**

Remove the entire `fn attribute_conflicts(...)` from `scene.rs` (the leave-one-out pass, ~40 lines). Keep `solve_diags` (still used by `placement_would_conflict`) and keep `placement_would_conflict` unchanged.

- [ ] **Step 7: Add `conflicts: Vec::new()` to the other `Scene` literals**

Two `Scene { ... }` literals in `build_focus_scene` (the early-return empty scene and the final one) and the `Scene { nodes: vec![], ... }` in the `bounding_box_none_for_empty_scene` test each need `conflicts: Vec::new(),` (or `conflicts: vec![],`) added. `project_scene_node` etc. don't construct `Scene`. `Scene::default()` sites (app.rs) are unaffected (derive Default fills the new Vec).

- [ ] **Step 8: Keep `canvas.rs` compiling — stop reading `conflicting`**

In `canvas.rs`, `draw_relations_overlay` reads `rel.conflicting`. Change the `filter_map` closure to drop it and the color branch to a single calm slate (Task 4 replaces this whole method with the veil; this is only the minimal compile-keeping edit):

Replace the `Some((si, ri, rel.dir, rel.conflicting))` line with `Some((si, ri, rel.dir))` and change the tuple type to `Vec<(usize, usize, waml::syntax::Direction)>`. Replace the draw loop:

```rust
        for (si, ri, dir) in legs {
            let color = vec4(0.55, 0.62, 0.72, 0.45); // calm slate
            self.draw_relation_connector(cx, si, ri, dir, color);
        }
```

In the canvas test `relations_in_scope_keeps_only_relations_touching_dragged_or_target`, remove the `conflicting: false,` line from each of the three `SceneRelation { ... }` literals.

- [ ] **Step 9: Run the tests to verify they pass**

Run: `cargo test -p waml-editor --lib scene`
Expected: PASS (both new tests; the old attribution tests are gone).
Run: `cargo test -p waml-editor --lib`
Expected: PASS (the canvas `relations_in_scope` test compiles + passes without the field).

- [ ] **Step 10: Run the full gate**

Run: `cargo test --workspace`
Expected: PASS.
Run: `cargo clippy --workspace --all-targets -- -D warnings`
Expected: clean (watch for an unused `solve_diagram` import in scene.rs — remove it if flagged).

- [ ] **Step 11: Commit**

```bash
git add crates/waml-editor/src/scene.rs crates/waml-editor/src/canvas.rs
git commit -m "refactor(scene): drop Stage-4 attribution, carry solver conflict report"
```
