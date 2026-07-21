# Expandable Card Body Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A node card with many members renders at most 4 member rows plus a `▾ N more` footer; clicking the footer re-solves the diagram to show all members (footer becomes `▴ show less`), never touching the model.

**Architecture:** Expansion is ephemeral view state: an `App`-owned `HashSet<String>` of expanded node keys. The set is threaded down through scene build → `sizing::size_map` (so the *measured* hull is the collapsed-or-expanded card) and mirrored onto each `SceneNode.expanded`, so the solver, the draw path (`card::class_shape`), and the footer hit-test all measure the same shape. Truncation lives in `card::class_shape`, keyed off `node.expanded`. A footer click emits `GraphCanvasAction::ToggleExpand`; `App` flips the key and hands a rebuilt scene to a new `GraphCanvas::update_scene` that holds the camera and re-resolves selection by key.

**Tech Stack:** Rust 2021, `waml-editor` crate (native, makepad fork + taffy). No new dependencies.

## Global Constraints

- Work in this git worktree only; never edit the main checkout directly.
- Windows / PowerShell. Never bare `git stash`.
- `MAX_BODY_ROWS = 4`, a fixed `pub const` in `card/mod.rs`. Not styleable.
- Whole-body scope: attributes then operations are ONE ordered member list, ONE shared limit, ONE footer at the card bottom.
- Expansion state lives only in `App` (a `HashSet<String>` of node keys); it is never written back to the model. It is cleared when the open diagram changes and survives same-diagram rebuilds.
- Footer labels are exactly `▾ {N} more` (collapsed, `N == total - MAX_BODY_ROWS`) and `▴ show less` (expanded). Glyphs are `\u{25be}` (▾) and `\u{25b4}` (▴).
- Feature is named **expand / member-overflow**. Do NOT touch the unrelated authored `SceneNode.collapsed` directive.
- `build_focus_scene` passes `expanded = false` (locked; focus overflow is out of scope).
- Full gate for every task: `cargo test --workspace && pnpm -r test && pnpm lint && pnpm build`.

---

### Task 1: `SceneNode.expanded` field

Add the ephemeral per-node expand flag to the scene projection. Additive, defaults `false` like `emphasized` / `collapsed`. Every `SceneNode` literal must set it or the crate will not compile.

**Files:**
- Modify: `crates/waml-editor/src/scene.rs` (struct `SceneNode` ~:23; `project_scene_node` ~:100; synthetic fallback literal in `build_scene` ~:200; `build_focus_scene` literal ~:268; test helper is in `card/mod.rs`)
- Modify: `crates/waml-editor/src/card/mod.rs` (test helper `scene_node` ~:653)
- Test: `crates/waml-editor/src/scene.rs` (`#[cfg(test)] mod tests`)

**Interfaces:**
- Produces: `crate::scene::SceneNode` gains `pub expanded: bool`. `project_scene_node` and the synthetic fallback default it to `false`.

- [ ] **Step 1: Write the failing test**

Add to `scene.rs`'s `mod tests`:

```rust
    #[test]
    fn projected_node_defaults_to_not_expanded() {
        let model = mini();
        let node = model.nodes.iter().find(|n| n.key == "order").unwrap();
        let projected = project_scene_node(&model, node);
        assert!(!projected.expanded);
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p waml-editor scene::tests::projected_node_defaults_to_not_expanded`
Expected: FAIL to compile — `no field \`expanded\` on type \`SceneNode\``.

- [ ] **Step 3: Add the field and default it everywhere**

In `scene.rs`, add to `struct SceneNode` right after `pub collapsed: bool,`:

```rust
    /// Ephemeral view-state: whether the card shows all members (true) or is
    /// capped at `card::MAX_BODY_ROWS` with a `▾ N more` footer (false). Set from
    /// `App`'s expanded key-set in `build_scene`; never derived from the model.
    /// Defaults `false` (collapsed) everywhere the model projects a node.
    pub expanded: bool,
```

In `project_scene_node`, add `expanded: false,` after `collapsed: false,`.
In the synthetic fallback `SceneNode { .. }` literal inside `build_scene`, add `expanded: false,` after its `collapsed: false,`.
In `build_focus_scene`'s `SceneNode { .. }` literal, add `expanded: false,` after its `collapsed: false,`.

In `card/mod.rs`'s test helper `scene_node`, add `expanded: false,` after its `collapsed: false,`.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p waml-editor scene:: ; cargo test -p waml-editor card::`
Expected: PASS (compiles; new test green; existing card/scene tests unchanged).

- [ ] **Step 5: Run the full gate**

Run: `cargo test --workspace && pnpm -r test && pnpm lint && pnpm build`
Expected: PASS.

- [ ] **Step 6: Commit**

```powershell
git add crates/waml-editor/src/scene.rs crates/waml-editor/src/card/mod.rs
git commit -m "feat(card): add ephemeral SceneNode.expanded flag"
```

---

### Task 2: Truncation + footer in `card::class_shape`

Read `node.expanded`, treat attributes-then-operations as one ordered list of length `M`, keep the first `MAX_BODY_ROWS` when collapsed (regrouping kept rows back into Attributes/Operations compartments), keep all when expanded, and append a `Block::Footer` row whenever `M > MAX_BODY_ROWS`.

**Files:**
- Modify: `crates/waml-editor/src/card/mod.rs` (`enum Block` ~:47; add `MAX_BODY_ROWS`; `struct StyleSheet` ~:348; `mono_sheet` ~:369; `class_shape` ~:420)
- Test: `crates/waml-editor/src/card/mod.rs` (`#[cfg(test)] mod tests`)

**Interfaces:**
- Consumes: `SceneNode.expanded` (Task 1).
- Produces: `pub const MAX_BODY_ROWS: usize = 4`; `Block::Footer`; `StyleSheet.footer: TextStyle`. `class_shape` now emits at most `MAX_BODY_ROWS` member rows when `!expanded`, plus a footer row (`Block::Footer`, one accent Text leaf) when the full member count exceeds the cap. `measure` captures the footer rect automatically (any `block != Block::None` is pushed in `flatten` — no change needed there).

- [ ] **Step 1: Write the failing tests**

Add to `card/mod.rs`'s `mod tests` (the helpers `scene_node`, `attr`, `op`, `drawn`, `measure`, `class_shape`, `mono_sheet` already exist in that module):

```rust
    fn attrs_named(prefix: &str, n: usize) -> Vec<AttrRow> {
        (0..n).map(|i| attr(&format!("{prefix}{i}"), "Int", "+", "")).collect()
    }

    #[test]
    fn four_or_fewer_members_have_no_footer() {
        let n = scene_node("Big", vec![], attrs_named("f", 4));
        let placed = measure(&class_shape(&n, &mono_sheet()));
        assert!(!placed.blocks.iter().any(|b| b.block == Block::Footer));
    }

    #[test]
    fn collapsed_over_cap_keeps_four_rows_and_a_more_footer() {
        let n = scene_node("Big", vec![], attrs_named("f", 7));
        let s = drawn(&n);
        // First four kept, rest hidden.
        for i in 0..4 {
            assert!(s.contains(&format!("f{i}")), "f{i} should be kept");
        }
        for i in 4..7 {
            assert!(!s.contains(&format!("f{i}")), "f{i} should be hidden");
        }
        // Footer counts the hidden members (7 - 4 = 3).
        assert!(s.contains(&"\u{25be} 3 more".to_string()));
        let placed = measure(&class_shape(&n, &mono_sheet()));
        assert!(placed.blocks.iter().any(|b| b.block == Block::Footer));
    }

    #[test]
    fn expanded_over_cap_shows_all_rows_and_a_show_less_footer() {
        let mut n = scene_node("Big", vec![], attrs_named("f", 7));
        n.expanded = true;
        let s = drawn(&n);
        for i in 0..7 {
            assert!(s.contains(&format!("f{i}")), "f{i} should be shown");
        }
        assert!(s.contains(&"\u{25b4} show less".to_string()));
    }

    #[test]
    fn footer_sits_below_the_last_compartment() {
        let n = scene_node("Big", vec![], attrs_named("f", 7));
        let placed = measure(&class_shape(&n, &mono_sheet()));
        let attrs = placed.blocks.iter().find(|b| b.block == Block::Attributes).unwrap();
        let footer = placed.blocks.iter().find(|b| b.block == Block::Footer).unwrap();
        assert!(footer.y >= attrs.y + attrs.h - 0.01, "footer must sit below attributes");
    }

    #[test]
    fn mid_list_truncation_regroups_kept_rows_into_compartments() {
        // 3 attributes + 3 operations, cap 4 -> keep all 3 attrs + first 1 op.
        let mut n = scene_node("Svc", vec![], attrs_named("a", 3));
        n.operations = vec![
            op("op0", Some(""), "void", "+"),
            op("op1", Some(""), "void", "+"),
            op("op2", Some(""), "void", "+"),
        ];
        let placed = measure(&class_shape(&n, &mono_sheet()));
        let roles: Vec<Block> = placed.blocks.iter().map(|b| b.block).collect();
        assert!(roles.contains(&Block::Attributes));
        assert!(roles.contains(&Block::Operations));
        assert!(roles.contains(&Block::Footer));
        let s = drawn(&n);
        assert!(s.contains(&"a0".to_string()) && s.contains(&"a2".to_string()));
        assert!(s.contains(&"op0".to_string()));
        assert!(!s.contains(&"op1".to_string()) && !s.contains(&"op2".to_string()));
        // 6 members - 4 cap = 2 hidden.
        assert!(s.contains(&"\u{25be} 2 more".to_string()));
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p waml-editor card::tests::collapsed_over_cap_keeps_four_rows_and_a_more_footer`
Expected: FAIL to compile — `no variant \`Footer\`` / `no field \`footer\`` (and the compiled tests would then fail: no truncation yet).

- [ ] **Step 3: Add `Block::Footer`, `MAX_BODY_ROWS`, and the `footer` style**

In `enum Block`, add `Footer` after `Operations`:

```rust
pub enum Block {
    None,
    Header,
    Attributes,
    Operations,
    Footer,
}
```

Directly above `pub fn class_shape`, add the cap constant:

```rust
/// Collapsed card body caps at this many member rows (attributes + operations
/// combined), then a footer row. Fixed; not styleable.
pub const MAX_BODY_ROWS: usize = 4;
```

In `struct StyleSheet`, add a field after `pub cardinality: TextStyle,`:

```rust
    /// The `▾ N more` / `▴ show less` overflow footer row.
    pub footer: TextStyle,
```

In `mono_sheet`'s returned `StyleSheet { .. }`, add after `cardinality: body(Token::Amber, Weight::Regular),`:

```rust
        footer: body(Token::Accent, Weight::Regular),
```

- [ ] **Step 4: Rewrite the compartment building in `class_shape`**

Keep the header block exactly as-is. Replace the two compartment blocks (the `if !node.attributes.is_empty() { .. }` and `if !node.operations.is_empty() { .. }` sections) so they build from truncated slices, and append the footer. The cell-building inner loops are unchanged — only the iterator source (`&node.attributes[..attrs_shown]` / `&node.operations[..ops_shown]`) and the guard (`> 0`) change. Insert this block where those two `if` sections were (between the header push and the final outer `Shape::Box`):

```rust
    // Member overflow: attributes then operations form one ordered list. When
    // collapsed and over the cap, keep only the first MAX_BODY_ROWS, regrouped
    // back into their compartments; expanded keeps all. A footer row appears
    // whenever the full list exceeds the cap.
    let total = node.attributes.len() + node.operations.len();
    let overflow = total > MAX_BODY_ROWS;
    let keep = if node.expanded || !overflow {
        total
    } else {
        MAX_BODY_ROWS
    };
    let attrs_shown = keep.min(node.attributes.len());
    let ops_shown = keep - attrs_shown;

    // Attributes compartment.
    if attrs_shown > 0 {
        let mut at_rows = Vec::new();
        for attr in &node.attributes[..attrs_shown] {
            let mut cells = Vec::new();
            if !attr.visibility.is_empty() {
                cells.push(Shape::Text {
                    text: attr.visibility.clone(),
                    style: sheet.marker,
                });
            }
            cells.push(Shape::Text {
                text: attr.name.clone(),
                style: sheet.name,
            });
            if !attr.ty.is_empty() {
                cells.push(Shape::Text {
                    text: ":".to_string(),
                    style: sheet.colon,
                });
                cells.push(Shape::Text {
                    text: attr.ty.clone(),
                    style: sheet.ty,
                });
            }
            if !attr.multiplicity.is_empty() {
                cells.push(Shape::Text {
                    text: format!("{{{}}}", attr.multiplicity),
                    style: sheet.cardinality,
                });
            }
            at_rows.push(Shape::Box {
                dir: Dir::Row,
                gap: sheet.row_gap,
                pad: Edges::ZERO,
                hidden: false,
                block: Block::None,
                children: cells,
            });
        }
        rows.push(Shape::Box {
            dir: Dir::Col,
            gap: sheet.rows_gap,
            pad: Edges::ZERO,
            hidden: false,
            block: Block::Attributes,
            children: at_rows,
        });
    }

    // Operations compartment: `<vis> <name>(<params>) : <ret>`. The name and its
    // parenthesized parameter list are a no-gap sub-box so they read as one token.
    if ops_shown > 0 {
        let mut op_rows = Vec::new();
        for op in &node.operations[..ops_shown] {
            let mut cells = Vec::new();
            if !op.visibility.is_empty() {
                cells.push(Shape::Text {
                    text: op.visibility.clone(),
                    style: sheet.marker,
                });
            }
            let mut sig = vec![Shape::Text {
                text: op.name.clone(),
                style: sheet.name,
            }];
            if let Some(params) = &op.params {
                sig.push(Shape::Text {
                    text: format!("({params})"),
                    style: sheet.colon,
                });
            }
            cells.push(Shape::Box {
                dir: Dir::Row,
                gap: 0.0,
                pad: Edges::ZERO,
                hidden: false,
                block: Block::None,
                children: sig,
            });
            if !op.ret.is_empty() {
                cells.push(Shape::Text {
                    text: ":".to_string(),
                    style: sheet.colon,
                });
                cells.push(Shape::Text {
                    text: op.ret.clone(),
                    style: sheet.colon,
                });
            }
            op_rows.push(Shape::Box {
                dir: Dir::Row,
                gap: sheet.row_gap,
                pad: Edges::ZERO,
                hidden: false,
                block: Block::None,
                children: cells,
            });
        }
        rows.push(Shape::Box {
            dir: Dir::Col,
            gap: sheet.rows_gap,
            pad: Edges::ZERO,
            hidden: false,
            block: Block::Operations,
            children: op_rows,
        });
    }

    // Overflow footer row: its own accent-mono control line.
    if overflow {
        let label = if node.expanded {
            "\u{25b4} show less".to_string()
        } else {
            format!("\u{25be} {} more", total - MAX_BODY_ROWS)
        };
        rows.push(Shape::Box {
            dir: Dir::Row,
            gap: 0.0,
            pad: Edges::ZERO,
            hidden: false,
            block: Block::Footer,
            children: vec![Shape::Text {
                text: label,
                style: sheet.footer,
            }],
        });
    }
```

- [ ] **Step 5: Run the card tests**

Run: `cargo test -p waml-editor card::`
Expected: PASS (new footer tests + all existing card tests).

- [ ] **Step 6: Run the full gate**

Run: `cargo test --workspace && pnpm -r test && pnpm lint && pnpm build`
Expected: PASS.

- [ ] **Step 7: Commit**

```powershell
git add crates/waml-editor/src/card/mod.rs
git commit -m "feat(card): cap body at MAX_BODY_ROWS with an expand footer"
```

---

### Task 3: Thread the expanded set through `sizing`

`size_of` measures the effective (collapsed-or-expanded) hull, and `size_map` picks each node's flag from the expanded key-set. The rect the solver lays out then equals the drawn card in both states.

**Files:**
- Modify: `crates/waml-editor/src/sizing.rs` (`size_of` ~:11; `size_map` ~:18)
- Test: `crates/waml-editor/src/sizing.rs` (`#[cfg(test)] mod tests`)

**Interfaces:**
- Consumes: `SceneNode.expanded` (Task 1); `card::MAX_BODY_ROWS` (Task 2).
- Produces: `pub fn size_of(model: &Model, node: &Node, expanded: bool) -> Size`; `pub fn size_map(model: &Model, diagram: &Diagram, expanded: &std::collections::HashSet<String>) -> SizeMap`.

- [ ] **Step 1: Write the failing test**

Add to `sizing.rs`'s `mod tests` (`model_with_attrs`, `node0` already exist):

```rust
    #[test]
    fn collapsed_hull_is_shorter_than_expanded_for_many_members() {
        let model = model_with_attrs(8, "Int");
        let collapsed = size_of(&model, node0(&model), false);
        let expanded = size_of(&model, node0(&model), true);
        assert!(expanded.h > collapsed.h, "expanded card must be taller");
    }
```

Also update the existing `size_of` / `size_map` call sites in this test module so the module compiles:
- `size_of_measures_the_card_hull`: change to `size_of(&model, node0(&model), false)`, and change `hull` to measure the collapsed card (its 2-attr node is under the cap, so `expanded` is irrelevant — leave `hull` as-is; it already measures the full un-truncated card, which for ≤4 members equals the collapsed card).
- `hull_grows_taller_with_more_attribute_rows`: `size_of(&one, node0(&one), false)` and `size_of(&three, node0(&three), false)`.
- `hull_grows_wider_with_a_longer_attribute_type`: `size_of(&short, node0(&short), false)` and `size_of(&long, node0(&long), false)`.
- `node_without_attributes_still_has_positive_hull`: `size_of(&model, node0(&model), false)`.
- `size_map_covers_every_resolved_member_with_positive_sizes`: `size_map(&model, diagram, &HashSet::new())`.
- `size_map_matches_card_hull_for_each_member`: `size_map(&model, diagram, &HashSet::new())` and, in the loop, `size_of(&model, node, false)`.

Add `use std::collections::HashSet;` to the test module's `use super::*;` block.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p waml-editor sizing::tests::collapsed_hull_is_shorter_than_expanded_for_many_members`
Expected: FAIL to compile — `size_of` takes 2 arguments / `size_map` takes 2 arguments.

- [ ] **Step 3: Add the `expanded` parameters**

Replace `size_of`:

```rust
/// Size one node for the solver by measuring its projected card hull in its
/// effective (collapsed-or-expanded) state. The rect the solver lays out then
/// equals the card the renderer draws, so card text lands exactly inside its box.
pub fn size_of(model: &Model, node: &Node, expanded: bool) -> Size {
    let mut scene_node = crate::scene::project_scene_node(model, node);
    scene_node.expanded = expanded;
    let (w, h) = crate::card::card_size(&scene_node, &crate::card::mono_sheet());
    Size { w, h }
}
```

Replace `size_map`:

```rust
/// Build a `SizeMap` for every diagram member that resolves to a classifier
/// node, measuring each in its effective state per the `expanded` key-set.
pub fn size_map(
    model: &Model,
    diagram: &Diagram,
    expanded: &std::collections::HashSet<String>,
) -> SizeMap {
    use std::collections::BTreeMap;
    let lookup: BTreeMap<&str, &Node> = model.nodes.iter().map(|n| (n.key.as_str(), n)).collect();

    let mut keys = Vec::new();
    collect_member_keys(&diagram.groups, &mut keys);

    let mut map = SizeMap::new();
    for key in keys {
        if let Some(node) = lookup.get(key.as_str()) {
            map.insert(key.clone(), size_of(model, node, expanded.contains(&key)));
        }
    }
    map
}
```

- [ ] **Step 4: Run the sizing tests**

Run: `cargo test -p waml-editor sizing::`
Expected: PASS.

- [ ] **Step 5: Commit**

The workspace does not yet compile (the `size_map` caller in `scene.rs` still passes two args); that is fixed in Task 4. Commit `sizing.rs` alone so the change is isolated, then run the gate at the end of Task 4.

```powershell
git add crates/waml-editor/src/sizing.rs
git commit -m "feat(sizing): thread expanded key-set into size_of/size_map"
```

---

### Task 4: Thread the expanded set through `build_scene` + `App`

Give `App` its `expanded: HashSet<String>`, change `build_scene` to accept it, mirror each solved node's flag onto `SceneNode.expanded`, and update both callers. `build_focus_scene` passes `false`.

**Files:**
- Modify: `crates/waml-editor/src/scene.rs` (`build_scene` ~:180; node-build loop ~:194; `build_focus_scene` ~:286 `card_size` call is unaffected)
- Modify: `crates/waml-editor/src/app.rs` (`struct App` ~:265; `sync_active_tab` build_scene call ~:325; `open_dir` build_scene call ~:572)
- Test: `crates/waml-editor/src/scene.rs` (`#[cfg(test)] mod tests`)

**Interfaces:**
- Consumes: `sizing::size_map(.., expanded)` (Task 3); `SceneNode.expanded` (Task 1).
- Produces: `pub fn build_scene(model: &Model, diagram: &Diagram, expanded: &std::collections::HashSet<String>) -> (Scene, Vec<Diagnostic>)`. `App` gains a private `expanded: std::collections::HashSet<String>` field (Task 6 mutates it).

- [ ] **Step 1: Write the failing test**

Add to `scene.rs`'s `mod tests`:

```rust
    #[test]
    fn build_scene_mirrors_the_expanded_flag_onto_its_node() {
        let model = mini();
        let mut expanded = std::collections::HashSet::new();
        expanded.insert("order".to_string());
        let (scene, _) = build_scene(&model, &model.diagrams[0], &expanded);
        let order = scene.nodes.iter().find(|n| n.key == "order").unwrap();
        let customer = scene.nodes.iter().find(|n| n.key == "customer").unwrap();
        assert!(order.expanded, "order was in the expanded set");
        assert!(!customer.expanded, "customer was not");
    }
```

Update every existing `build_scene(&model, ..)` call in this test module to pass `&std::collections::HashSet::new()` as the third argument. The affected tests are: `scene_has_both_nodes_with_titles`, `build_scene_nodes_carry_attribute_rows`, `scene_nodes_carry_their_model_element_type`, `scene_edge_endpoints_match_node_rects`, `layout_places_order_left_of_customer`, `bounding_box_covers_all_nodes`. (`build_focus_scene` calls are unchanged.)

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p waml-editor scene::tests::build_scene_mirrors_the_expanded_flag_onto_its_node`
Expected: FAIL to compile — `build_scene` takes 2 arguments.

- [ ] **Step 3: Change `build_scene`**

Change the signature and the internal `size_map` call, and mirror the flag onto each node. New signature line:

```rust
pub fn build_scene(
    model: &Model,
    diagram: &Diagram,
    expanded: &std::collections::HashSet<String>,
) -> (Scene, Vec<Diagnostic>) {
```

Inside it, change the sizing call:

```rust
    let sizes = crate::sizing::size_map(model, diagram, expanded);
```

In the node-build loop, after `node.collapsed = flags.collapsed;`, add:

```rust
        node.expanded = expanded.contains(key);
```

(`key` is the loop binding `for (key, rect) in &solved.nodes`, a `&String`; `contains` takes `&String` fine.)

`build_focus_scene` is unchanged — its literal already sets `expanded: false` (Task 1), and it calls `card_size` directly, never `build_scene`.

- [ ] **Step 4: Add the `App` field and update both callers**

In `app.rs`'s `struct App`, add after the `fps_meter` field:

```rust
    /// Ephemeral set of node keys whose card body is expanded (all members
    /// shown) rather than capped at `card::MAX_BODY_ROWS`. Never persisted to the
    /// model; cleared when the open diagram changes, held across same-diagram
    /// rebuilds. See `GraphCanvasAction::ToggleExpand` handling.
    #[rust]
    expanded: std::collections::HashSet<String>,
```

In `sync_active_tab`, change the diagram-tab build call (`.map(|d| build_scene(&self.model, d))`) to:

```rust
                    .map(|d| build_scene(&self.model, d, &self.expanded));
```

In `open_dir`, change `let (scene, diags) = build_scene(&self.model, diagram);` to:

```rust
                let (scene, diags) = build_scene(&self.model, diagram, &self.expanded);
```

- [ ] **Step 5: Run the scene tests + full gate**

Run: `cargo test -p waml-editor scene:: && cargo test --workspace && pnpm -r test && pnpm lint && pnpm build`
Expected: PASS (workspace compiles again).

- [ ] **Step 6: Commit**

```powershell
git add crates/waml-editor/src/scene.rs crates/waml-editor/src/app.rs
git commit -m "feat(scene): thread expanded key-set through build_scene + App"
```

---

### Task 5: Canvas footer hit-test, `ToggleExpand` action, key-tracked selection, `update_scene`

A sub-slop click that lands in a node's footer band emits `ToggleExpand` and is consumed (no selection change). The canvas tracks the selected node's key so a same-diagram re-solve keeps the highlight, and `update_scene` swaps the scene without refitting the camera.

**Files:**
- Modify: `crates/waml-editor/src/canvas.rs` (`enum GraphCanvasAction` ~:295; `struct GraphCanvas` add `selected_key` ~:191; `FingerUp` primary handler ~:339; `set_scene` ~:619; `set_focus` ~:630; add `footer_screen_rect`, `selection_index`, `update_scene`)
- Test: `crates/waml-editor/src/canvas.rs` (`#[cfg(test)] mod tests`)

**Interfaces:**
- Consumes: `card::Block::Footer`, `card::measure`, `card::class_shape`, `card::mono_sheet` (Task 2); `SceneNode` (Task 1).
- Produces: `GraphCanvasAction::ToggleExpand { key: String }`; `pub fn footer_screen_rect(node: &crate::scene::SceneNode, screen: Rect, zoom: f64) -> Option<Rect>`; `pub fn update_scene(&mut self, cx: &mut Cx, scene: Scene)`. `App` (Task 6) reads `ToggleExpand` via the existing `canvas_action` seam and calls `update_scene`.

- [ ] **Step 1: Write the failing tests**

Add to `canvas.rs`'s `mod tests` (it already has `use super::*;` and `use waml::solve::Rect as WorldRect;`):

```rust
    fn many_attr_node(key: &str, n: usize) -> crate::scene::SceneNode {
        use crate::inspector::AttrRow;
        use waml::model::{ElementType, UmlMetaclass};
        crate::scene::SceneNode {
            key: key.to_string(),
            title: "N".to_string(),
            element_type: ElementType::Uml(UmlMetaclass::Class),
            stereotypes: vec![],
            attributes: (0..n)
                .map(|i| AttrRow {
                    name: format!("f{i}"),
                    ty: "Int".to_string(),
                    multiplicity: String::new(),
                    visibility: "+".to_string(),
                })
                .collect(),
            operations: vec![],
            header: crate::scene::HeaderStyle::Plain,
            ports: false,
            rect: WorldRect { x: 0.0, y: 0.0, w: 0.0, h: 0.0 },
            emphasized: false,
            collapsed: false,
            expanded: false,
        }
    }

    #[test]
    fn footer_rect_present_for_an_over_cap_node_and_absent_otherwise() {
        let screen = Rect { pos: dvec2(0.0, 0.0), size: dvec2(200.0, 200.0) };
        let over = many_attr_node("big", 7);
        let under = many_attr_node("small", 2);
        assert!(footer_screen_rect(&over, screen, 1.0).is_some());
        assert!(footer_screen_rect(&under, screen, 1.0).is_none());
    }

    #[test]
    fn a_point_in_the_footer_band_is_inside_the_footer_rect() {
        let screen = Rect { pos: dvec2(10.0, 20.0), size: dvec2(200.0, 200.0) };
        let node = many_attr_node("big", 7);
        let fr = footer_screen_rect(&node, screen, 1.0).unwrap();
        let mid = dvec2(fr.pos.x + fr.size.x * 0.5, fr.pos.y + fr.size.y * 0.5);
        assert!(fr.contains(mid));
        // A point well above the footer (in the header) is not in the footer.
        assert!(!fr.contains(dvec2(mid.x, screen.pos.y + 1.0)));
    }

    #[test]
    fn selection_index_resolves_by_key_and_clears_on_miss() {
        let a = many_attr_node("a", 1);
        let b = many_attr_node("b", 1);
        let nodes = vec![a, b];
        assert_eq!(selection_index(&nodes, Some("b")), Some(1));
        assert_eq!(selection_index(&nodes, Some("gone")), None);
        assert_eq!(selection_index(&nodes, None), None);
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p waml-editor canvas::tests::footer_rect_present_for_an_over_cap_node_and_absent_otherwise`
Expected: FAIL to compile — `footer_screen_rect` / `selection_index` not found, `SceneNode` missing `expanded`... (`expanded` exists from Task 1; the missing items are the two new fns).

- [ ] **Step 3: Add the `ToggleExpand` action variant**

In `enum GraphCanvasAction`, add after `NodeDeselect,`:

```rust
    /// A primary click landed on a node's overflow footer band: toggle its card
    /// expansion. Consumed — no selection change. Carries the `SceneNode::key`.
    ToggleExpand { key: String },
```

- [ ] **Step 4: Add the `selected_key` field**

In `struct GraphCanvas`, add after the `selected: Option<usize>,` field (keep its doc comment above `selected`):

```rust
    /// Key of the click-selected node, tracked alongside `selected` so a
    /// same-diagram re-solve (`update_scene`) can re-find the node by key after
    /// its index shifts. Reset to `None` whenever the scene is replaced.
    #[rust]
    selected_key: Option<String>,
```

- [ ] **Step 5: Add the pure helpers**

Add near `node_at` (module-level fns, above or below it):

```rust
/// Screen-space rect of `node`'s overflow footer band, or `None` when the card
/// has no footer (member count at or under `card::MAX_BODY_ROWS`). Measures the
/// same box-tree `draw_card` draws, so the hit-band matches the drawn control.
/// Pure (takes the node + its on-screen rect + zoom), so it is unit-testable
/// without a GPU, mirroring `node_at` / `is_click`.
pub fn footer_screen_rect(
    node: &crate::scene::SceneNode,
    screen: Rect,
    zoom: f64,
) -> Option<Rect> {
    use crate::card::{self, Block};
    let placed = card::measure(&card::class_shape(node, &card::mono_sheet()));
    let f = placed.blocks.iter().find(|b| b.block == Block::Footer)?;
    Some(Rect {
        pos: dvec2(screen.pos.x + f.x * zoom, screen.pos.y + f.y * zoom),
        size: dvec2(f.w * zoom, f.h * zoom),
    })
}

/// Index of the node whose key equals `key`, or `None` (missing key / `None`).
/// Used by `update_scene` to re-resolve the selection after a re-solve reorders
/// the node vector. Pure, for a GPU-free test.
fn selection_index(nodes: &[crate::scene::SceneNode], key: Option<&str>) -> Option<usize> {
    let key = key?;
    nodes.iter().position(|n| n.key == key)
}
```

- [ ] **Step 6: Wire the footer hit-test into `FingerUp` and track the key**

In the `Hit::FingerUp(fe) if fe.is_primary_hit()` arm, replace the `match node_at(..) { Some(i) => { .. } None => { .. } }` body with:

```rust
                        match node_at(&rects, &self.camera, self.view_rect, fe.abs) {
                            Some(i) => {
                                // Clone the node so the footer measure + redraw
                                // don't hold an immutable borrow of the scene.
                                let node = self.scene.nodes[i].clone();
                                let (lx, ly) =
                                    self.camera.world_to_local(node.rect.x, node.rect.y);
                                let screen = Rect {
                                    pos: dvec2(
                                        self.view_rect.pos.x + lx,
                                        self.view_rect.pos.y + ly,
                                    ),
                                    size: dvec2(
                                        node.rect.w * self.camera.zoom,
                                        node.rect.h * self.camera.zoom,
                                    ),
                                };
                                let footer_hit =
                                    footer_screen_rect(&node, screen, self.camera.zoom)
                                        .map(|fr| fr.contains(fe.abs))
                                        .unwrap_or(false);
                                if footer_hit {
                                    // Consumed: toggle expansion, no selection change.
                                    cx.widget_action(
                                        uid,
                                        GraphCanvasAction::ToggleExpand { key: node.key.clone() },
                                    );
                                } else {
                                    self.selected = Some(i);
                                    self.selected_key = Some(node.key.clone());
                                    cx.widget_action(
                                        uid,
                                        GraphCanvasAction::NodeSelect { key: node.key.clone() },
                                    );
                                }
                            }
                            None => {
                                self.selected = None;
                                self.selected_key = None;
                                cx.widget_action(uid, GraphCanvasAction::NodeDeselect);
                            }
                        }
```

- [ ] **Step 7: Add `update_scene`; clear `selected_key` in `set_scene` / `set_focus`**

In `set_scene`, add after `self.selected = None;`:

```rust
        self.selected_key = None;
```

In `set_focus`, add after `self.selected = None;`:

```rust
        self.selected_key = None;
```

Add a new method next to `set_scene` (inside `impl GraphCanvas`):

```rust
    /// Swap the scene for a same-diagram re-solve (e.g. an expand toggle). Unlike
    /// `set_scene`, this holds the camera (`fitted` and `focus_mode` untouched)
    /// and re-resolves the selection by key, so the inspector highlight survives
    /// even though the node's index may have shifted.
    pub fn update_scene(&mut self, cx: &mut Cx, scene: Scene) {
        self.scene = scene;
        self.selected = selection_index(&self.scene.nodes, self.selected_key.as_deref());
        if self.selected.is_none() {
            self.selected_key = None;
        }
        self.draw_bg.redraw(cx);
    }
```

- [ ] **Step 8: Run the canvas tests + full gate**

Run: `cargo test -p waml-editor canvas:: && cargo test --workspace && pnpm -r test && pnpm lint && pnpm build`
Expected: PASS.

- [ ] **Step 9: Commit**

```powershell
git add crates/waml-editor/src/canvas.rs
git commit -m "feat(canvas): footer hit-test, ToggleExpand, key-tracked selection, update_scene"
```

---

### Task 6: Wire `ToggleExpand` in `App` + clear expansion on diagram change

Handle the new canvas action: flip the key in `App.expanded`, rebuild the current diagram scene with the updated set, and hand it to `update_scene`. Clear the set at the two diagram-change seams.

**Files:**
- Modify: `crates/waml-editor/src/app.rs` (`canvas_menu` match ~:1207; `switch_diagram` ~:413; `open_dir` ~:526)

**Interfaces:**
- Consumes: `GraphCanvasAction::ToggleExpand` + `GraphCanvas::update_scene` (Task 5); `build_scene(.., &expanded)` (Task 4).

- [ ] **Step 1: Clear `expanded` at the diagram-change seams**

In `switch_diagram`, immediately after the early-return `let Some(diagram) = .. else { .. };` guard and before `let base_id = ..`, add:

```rust
        // A new diagram is being shown in the base tab: drop stale expansion
        // (keyed by node key, which may not exist in the new diagram).
        self.expanded.clear();
```

In `open_dir`, after `self.model = model;` add:

```rust
        // Fresh model: no node keys carry over, so clear expansion state.
        self.expanded.clear();
```

- [ ] **Step 2: Handle `ToggleExpand` in the canvas action match**

In `handle_actions`, in the `match canvas_menu { .. }`, add a new arm before `_ => {}`:

```rust
            Some(crate::canvas::GraphCanvasAction::ToggleExpand { key }) => {
                if !self.expanded.remove(&key) {
                    self.expanded.insert(key);
                }
                // Re-solve the current diagram with the updated set; update_scene
                // holds the camera and re-resolves the selection by key.
                if let Some(active) = self.tabs.active_tab().cloned() {
                    if active.kind == TabKind::Diagram {
                        if let Some(diagram) =
                            self.model.diagrams.iter().find(|d| d.key == active.key)
                        {
                            let (scene, diags) =
                                build_scene(&self.model, diagram, &self.expanded);
                            for d in &diags {
                                log!("diagnostic: {d:?}");
                            }
                            if let Some(mut canvas) = self
                                .ui
                                .widget(cx, ids!(canvas))
                                .borrow_mut::<crate::canvas::GraphCanvas>()
                            {
                                canvas.update_scene(cx, scene);
                            }
                        }
                    }
                }
                return;
            }
```

- [ ] **Step 3: Run the app tests + full gate**

Run: `cargo test -p waml-editor && cargo test --workspace && pnpm -r test && pnpm lint && pnpm build`
Expected: PASS.

- [ ] **Step 4: Manual smoke (optional, not a gate)**

Run the native editor on a fixture whose classifier has >4 members and confirm: the card shows 4 rows + `▾ N more`; clicking the footer expands it and reroutes edges; the footer becomes `▴ show less`; the camera and inspector selection hold; switching diagrams resets expansion.

Run: `pwsh scripts/run-native.ps1` (or the `run` skill) on a suitable model.

- [ ] **Step 5: Commit**

```powershell
git add crates/waml-editor/src/app.rs
git commit -m "feat(app): toggle card expansion + reset on diagram change"
```

---

## Self-Review

**Spec coverage:**
- Decisions §1 (whole-body, combined list, one footer): Task 2 (`total`, single footer). ✓
- §2 (`MAX_BODY_ROWS = 4` const, not styleable): Task 2. ✓
- §3 (App-owned `HashSet<String>`, cleared on diagram change, survives edit-rebuilds): Task 4 (field) + Task 6 (clear at `switch_diagram`/`open_dir`, held elsewhere). ✓
- §4 (footer click toggles only, consumed, no select change): Task 5 (`footer_hit` branch emits `ToggleExpand`, skips `NodeSelect`) + Task 6. ✓
- §5 (expansion re-solves; collapsed is canonical size): Tasks 3–4 thread the flag into `size_map`; Task 6 rebuilds via `build_scene` + `update_scene`. ✓
- Architecture §1 threading (App → scene build → size_of → SceneNode.expanded mirror): Tasks 1, 3, 4. ✓
- §2 truncation + regroup + `Block::Footer` captured by `measure`: Task 2. ✓
- §3 `footer_screen_rect` pure helper + consume: Task 5. ✓
- §4 `update_scene` (holds camera, re-resolves selection by key; canvas tracks the key): Task 5. ✓
- Testing bullets (card ≤4/over-cap/expanded/footer-below/mid-list regroup; sizing collapsed<expanded; canvas footer-hit + selection-by-key): Tasks 2, 3, 5. ✓
- `build_focus_scene` passes `false` (out of scope): unchanged by Task 1 default; noted. ✓
- Out-of-scope items (no persistence, no per-compartment limits, no styleable N, no focus overflow, no animation): none implemented. ✓

**Placeholder scan:** No TBD / "add error handling" / vague steps — every code step carries full code.

**Type consistency:** `size_of(model, node, bool)`, `size_map(model, diagram, &HashSet<String>)`, `build_scene(model, diagram, &HashSet<String>)`, `GraphCanvasAction::ToggleExpand { key: String }`, `footer_screen_rect(&SceneNode, Rect, f64) -> Option<Rect>`, `selection_index(&[SceneNode], Option<&str>) -> Option<usize>`, `update_scene(&mut self, &mut Cx, Scene)`, `Block::Footer`, `StyleSheet.footer`, `MAX_BODY_ROWS`, `SceneNode.expanded` — all names used consistently across tasks that produce and consume them.
