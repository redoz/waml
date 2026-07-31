# Sequence Lifeline Title Label Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Display and measure sequence lifeline heads with the authored title only, while keeping the resolved reference key for non-display behavior.

**Architecture:** Keep `SeqNode::Lifeline::ref_` unchanged. Remove reference-key formatting at the two display boundaries: interaction measurement in `waml` and scene projection in `waml-editor`.

**Tech Stack:** Rust, Cargo test harness, WAML interaction solver, WAML editor behavior canvas

## Global Constraints

- Use ASD-STE100 Simplified Technical English.
- Do not change source syntax, model serialization, reference resolution, diagnostics, navigation, selection, or accent styling.
- Use the authored `SeqNode::Lifeline::title` as the full visible and measured head label.
- Prefix shell commands with `rtk`.

---

### Task 1: Use the Authored Lifeline Title

**Files:**
- Modify: `crates/waml/src/solve/interaction.rs:112-129`
- Test: `crates/waml/tests/interaction_solver_golden.rs`
- Test: `crates/waml/tests/fixtures/behavior/sequence-nested/sequence.golden.txt`
- Modify: `crates/waml-editor/src/behavior_doc_view.rs:217-269`
- Test: `crates/waml-editor/src/behavior_doc_view.rs:637-665`

**Interfaces:**
- Consumes: `SeqNode::Lifeline { id, title, ref_, .. }`
- Produces: a `SizeMap` whose `lifeline:<id>` entry uses `title`, and a `LifelineGeo::label` equal to `title`
- Preserves: `LifelineGeo::bucket` lookup through `ref_` and `lifeline_ref_key` navigation through `ref_`

- [x] **Step 1: Add the failing measurement regression test**

Add this test to `crates/waml/tests/interaction_solver_golden.rs`:

```rust
#[test]
fn resolved_lifeline_head_is_measured_from_title_only() {
    fn doc(ref_: Option<&str>) -> SequenceDoc {
        SequenceDoc {
            key: "sequence".into(),
            title: "Sequence".into(),
            describes: None,
            nodes: vec![SeqNode::Lifeline {
                id: "author".into(),
                title: "Author".into(),
                alias: None,
                ref_: ref_.map(str::to_string),
            }],
            edges: Vec::new(),
            items: Vec::new(),
        }
    }

    let cfg = InteractionConfig::default();
    let resolved = measure_interaction(
        &doc(Some("architecture/concepts/workflows/author")),
        &cfg,
    );
    let unresolved = measure_interaction(&doc(None), &cfg);

    assert_eq!(resolved["lifeline:author"], unresolved["lifeline:author"]);
}
```

- [x] **Step 2: Add the failing scene-label regression test**

Add this test after `interaction_doc_with_lifeline` in
`crates/waml-editor/src/behavior_doc_view.rs`:

```rust
#[test]
fn resolved_lifeline_scene_label_uses_title_only() {
    let doc = interaction_doc_with_lifeline(
        "Author",
        Some("architecture/concepts/workflows/author"),
    );

    let (scene, _) = build_interaction_scene(&waml::model::Model::default(), &doc);

    let BehaviorScene::Interaction { lifelines, .. } = scene else {
        panic!("expected interaction scene");
    };
    assert_eq!(lifelines[0].label, "Author");
}
```

- [x] **Step 3: Run both focused tests and verify RED**

Run:

```bash
rtk cargo test -p waml --test interaction_solver_golden resolved_lifeline_head_is_measured_from_title_only
rtk cargo test -p waml-editor resolved_lifeline_scene_label_uses_title_only
```

Expected:

- The measurement test fails because the resolved width is larger.
- The scene-label test fails because the label includes
  `:architecture/concepts/workflows/author`.

- [x] **Step 4: Measure the authored title only**

In `measure_interaction`, stop destructuring `ref_` and pass `title` directly
to `sizing::text_width`:

```rust
if let SeqNode::Lifeline { id, title, .. } = node {
    let w = sizing::text_width(title, cfg.font_size, Font::SansSemiBold)
        + cfg.head_pad_x * 2.0;
```

Keep the existing height calculation and size-map insertion unchanged.

- [x] **Step 5: Display the authored title only**

In `build_interaction_scene`, remove the resolved reference from the label
formatting but keep it for the accent bucket:

```rust
let (title, ref_) = lifeline_nodes
    .get(l.id.as_str())
    .copied()
    .unwrap_or((l.id.as_str(), None));
let label = title.to_string();
let bucket = ref_
    .and_then(|r| model.node(r))
    .map(|n| crate::accent::tree_kind_bucket(crate::tree::kind_of(&n.ty)))
    .unwrap_or(AccentBucket::Unknown);
```

- [x] **Step 6: Run both focused tests and verify GREEN**

Run:

```bash
rtk cargo test -p waml --test interaction_solver_golden resolved_lifeline_head_is_measured_from_title_only
rtk cargo test -p waml-editor resolved_lifeline_scene_label_uses_title_only
```

Expected: both tests pass.

- [x] **Step 7: Update the expected title-only golden layout**

Update `sequence.golden.txt` with the solver output from the new head widths:

```text
lifeline a head @ 0,0 40x41 stem x=20 41..742
lifeline b head @ 88,0 39x41 stem x=107 41..742
lifeline c head @ 175,0 39x41 stem x=195 41..742
lifeline d head @ 262,262 40x41 stem x=282 302..702 destroyed
```

Keep the message, activation, and fragment coordinates from the same solver
output so the golden fixture describes one consistent layout.

- [x] **Step 8: Run crate-wide regression tests**

Run:

```bash
rtk cargo test -p waml
rtk cargo test -p waml-editor
rtk cargo clippy -p waml -p waml-editor --all-targets -- -D warnings
```

Expected: all tests and Clippy pass without warnings.

- [x] **Step 9: Commit the implementation**

```bash
rtk git add crates/waml/src/solve/interaction.rs crates/waml/tests/interaction_solver_golden.rs crates/waml/tests/fixtures/behavior/sequence-nested/sequence.golden.txt crates/waml-editor/src/behavior_doc_view.rs docs/superpowers/plans/2026-07-31-sequence-lifeline-title-label.md
rtk git commit -m "fix(sequence): show lifeline title only"
```
