### Task 4: Veil renderer (hatched keep-out + grey scrim)

**Files:**
- Modify: `crates/waml-editor/src/canvas.rs` (add `ConstraintVeil` pen + `draw_veil` field; add pure `veil_band` helper + test; rewrite `draw_relations_overlay` to draw veils; delete `draw_relation_connector`, the drag-overlay relation-leg block, `relations_in_scope` + its test; retire the debug group-bounds outline)

**Interfaces:**
- Consumes (from Task 3): `crate::veil::{keep_out, in_keep_out, desaturated_cards, distance_fade, KeepOut}`.
- Produces (used by Task 5/6): `GraphCanvas::draw_veil_for(&mut self, cx, subject_idx, reference_idx, dir)` — draws one relation's veil + desaturation scrim. `veil_band(reference_screen, view, dir, reach) -> Rect` (pure, screen-space).
- Unchanged: `dir_word` STAYS (the DSL readout at the bottom of `draw_drag_overlay` still uses it). The compass (`draw_compass`) and drag-ghost drawing are untouched.

This task's one pure TDD seam is `veil_band` (screen-space anchored band, GPU-free, mirrors `segment_quad`). The shader pen art is the single place that may need fork shader-VM tweaks under interactive sign-off (deferred) — the gate requires only that it compiles and the pure test + workspace tests pass.

---

- [ ] **Step 1: Write the failing `veil_band` test**

Add to the `#[cfg(test)] mod tests` in `canvas.rs` (which already imports `super::*`):

```rust
#[test]
fn veil_band_anchors_and_clamps_per_direction() {
    // reference screen rect, view rect, reach.
    let reference = Rect { pos: dvec2(200.0, 100.0), size: dvec2(180.0, 80.0) };
    let view = Rect { pos: dvec2(0.0, 0.0), size: dvec2(1000.0, 700.0) };
    let reach = 300.0;
    use waml::syntax::Direction::*;

    // left of: band starts at the reference LEFT edge, extends right `reach`,
    // spans the full view height (y unlocked).
    let b = veil_band(reference, view, LeftOf, reach);
    assert_eq!(b.pos.x, 200.0);
    assert_eq!(b.size.x, 300.0);
    assert_eq!(b.pos.y, 0.0);
    assert_eq!(b.size.y, 700.0);

    // right of: band ends at the reference RIGHT edge (380), extends left `reach`.
    let b = veil_band(reference, view, RightOf, reach);
    assert_eq!(b.pos.x + b.size.x, 380.0);
    assert_eq!(b.size.x, 300.0);

    // above: band starts at the reference TOP edge, extends down `reach`, x unlocked.
    let b = veil_band(reference, view, Above, reach);
    assert_eq!(b.pos.y, 100.0);
    assert_eq!(b.size.y, 300.0);
    assert_eq!(b.pos.x, 0.0);
    assert_eq!(b.size.x, 1000.0);

    // above left of: BOTH axes locked to reach off the top-left corner.
    let b = veil_band(reference, view, AboveLeft, reach);
    assert_eq!((b.pos.x, b.pos.y, b.size.x, b.size.y), (200.0, 100.0, 300.0, 300.0));
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p waml-editor --lib canvas::tests::veil_band`
Expected: FAIL to compile — `veil_band` undefined.

- [ ] **Step 3: Add the pure `veil_band` helper**

Add near the other pure helpers in `canvas.rs` (e.g. just after `segment_quad`):

```rust
/// Screen-space fill rect for a veil: the keep-out region anchored to the
/// reference's screen rect (spec §2 mapping), clamped to `reach` px on each locked
/// axis and to the `view` bounds on the unlocked axis. Cardinal ⇒ one locked axis;
/// diagonal ⇒ both. Pure, GPU-free (unit-testable like `segment_quad`).
fn veil_band(reference: Rect, view: Rect, dir: waml::syntax::Direction, reach: f64) -> Rect {
    use waml::syntax::Direction::*;
    let (x0, xw) = match dir {
        LeftOf | AboveLeft | BelowLeft => (reference.pos.x, reach),
        RightOf | AboveRight | BelowRight => (reference.pos.x + reference.size.x - reach, reach),
        Above | Below => (view.pos.x, view.size.x),
    };
    let (y0, yh) = match dir {
        Above | AboveLeft | AboveRight => (reference.pos.y, reach),
        Below | BelowLeft | BelowRight => (reference.pos.y + reference.size.y - reach, reach),
        LeftOf | RightOf => (view.pos.y, view.size.y),
    };
    Rect { pos: dvec2(x0, y0), size: dvec2(xw, yh) }
}

/// Per-direction alpha-ramp uniforms for `ConstraintVeil`: `(ramp, bias)` so the
/// shader's `t = clamp(max(pos·ramp.axis + bias.axis), 0, 1)` runs 0 at the anchor
/// edge/corner to 1 at the far side (the distance fade). The unlocked axis is
/// biased far negative so `max` ignores it. Pure.
fn veil_ramp(dir: waml::syntax::Direction) -> ([f32; 2], [f32; 2]) {
    use waml::syntax::Direction::*;
    match dir {
        LeftOf => ([1.0, 0.0], [0.0, -9.0]),
        RightOf => ([-1.0, 0.0], [1.0, -9.0]),
        Above => ([0.0, 1.0], [-9.0, 0.0]),
        Below => ([0.0, -1.0], [-9.0, 1.0]),
        AboveLeft => ([1.0, 1.0], [0.0, 0.0]),
        AboveRight => ([-1.0, 1.0], [1.0, 0.0]),
        BelowLeft => ([1.0, -1.0], [0.0, 1.0]),
        BelowRight => ([-1.0, -1.0], [1.0, 1.0]),
    }
}
```

- [ ] **Step 4: Add the `ConstraintVeil` pen to the `script_mod!` DSL**

In `canvas.rs`'s `script_mod!` block, add a pen definition alongside `EdgeLine`/`EdgeMarker` (before `mod.widgets.GraphCanvas`):

```rust
    // Constraint veil pen: a faint grey wash + 45deg hatch over a keep-out
    // region, distance-faded from the anchor edge (spec §2). `ramp`/`bias`
    // orient the fade; `hatch_px` sets stripe spacing. Alpha rides self.color.w.
    mod.draw.ConstraintVeil = mod.draw.DrawColor{
        ramp: uniform(vec2(1.0, 0.0))
        bias: uniform(vec2(0.0, 0.0))
        hatch_px: uniform(9.0)
        pixel: fn() {
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            let p = self.pos * self.rect_size
            let s = self.hatch_px
            let d = abs(fract((p.x + p.y) / s) - 0.5) * s
            let line = 1.0 - clamp(d - 1.0, 0.0, 1.0)
            let ax = self.pos.x * self.ramp.x + self.bias.x
            let ay = self.pos.y * self.ramp.y + self.bias.y
            let t = clamp(max(ax, ay), 0.0, 1.0)
            let fade = 1.0 - t
            let a = self.color.w * (0.22 + 0.55 * line) * fade
            sdf.rect(0.0, 0.0, self.rect_size.x, self.rect_size.y)
            sdf.fill(vec4(self.color.x, self.color.y, self.color.z, a))
            return sdf.result
        }
    }
```

Inside the `mod.widgets.GraphCanvas` body, seed the pen instance next to the other `draw_*` seeds:

```rust
        draw_veil: mod.draw.ConstraintVeil{ color: vec4(0.42, 0.47, 0.54, 1.0) }
```

Add the field to the `GraphCanvas` struct (next to `draw_rule`):

```rust
    #[redraw]
    #[live]
    draw_veil: DrawColor,
```

- [ ] **Step 5: Add `draw_veil_for` and a distance-faded band draw**

Add a constant near the other screen-px constants:

```rust
/// How far (screen px) a veil hatch reaches from its anchor edge before fully
/// fading. Keeps a half-plane veil from flooding the canvas (spec §2).
const VEIL_REACH: f64 = 420.0;
```

Add this method to `impl GraphCanvas` (near `draw_relations_overlay`):

```rust
    /// Draw one placement relation's veil: a hatched grey keep-out anchored to the
    /// reference node's near edge, distance-faded, plus a desaturating scrim over
    /// every non-participant card inside it (spec §2). The two participants keep
    /// full colour. No connector line.
    fn draw_veil_for(
        &mut self,
        cx: &mut Cx2d,
        subject_idx: usize,
        reference_idx: usize,
        dir: waml::syntax::Direction,
    ) {
        let reference_screen = self.node_screen_rect(reference_idx);
        let band = veil_band(reference_screen, self.view_rect, dir, VEIL_REACH);
        // Clip the band to the view so we don't overdraw the whole window.
        let band = intersect_rect(band, self.view_rect);
        if band.size.x <= 0.5 || band.size.y <= 0.5 {
            return;
        }
        let (ramp, bias) = veil_ramp(dir);
        self.draw_veil.set_uniform(cx, live_id!(ramp), &ramp);
        self.draw_veil.set_uniform(cx, live_id!(bias), &bias);
        self.draw_veil.color = vec4(0.42, 0.47, 0.54, 1.0);
        self.draw_veil.draw_abs(cx, band);

        // Desaturation scrim over non-participant cards inside the keep-out.
        let subject_key = self.scene.nodes[subject_idx].key.clone();
        let reference_key = self.scene.nodes[reference_idx].key.clone();
        let reference_world = self.scene.nodes[reference_idx].rect;
        let cards: Vec<(String, waml::solve::Rect)> = self
            .scene
            .nodes
            .iter()
            .map(|n| (n.key.clone(), n.rect))
            .collect();
        let desats: Vec<String> =
            crate::veil::desaturated_cards(reference_world, dir, &cards, &subject_key, &reference_key)
                .into_iter()
                .map(str::to_string)
                .collect();
        for key in desats {
            if let Some(i) = self.scene.nodes.iter().position(|n| n.key == key) {
                let s = self.node_screen_rect(i);
                self.fill_rect(cx, s.pos.x, s.pos.y, s.size.x, s.size.y, vec4(0.62, 0.65, 0.70, 0.45));
            }
        }
    }
```

Add the pure rect-intersection helper near `veil_band`:

```rust
/// Axis-aligned intersection of two screen rects (empty size if disjoint). Pure.
fn intersect_rect(a: Rect, b: Rect) -> Rect {
    let x0 = a.pos.x.max(b.pos.x);
    let y0 = a.pos.y.max(b.pos.y);
    let x1 = (a.pos.x + a.size.x).min(b.pos.x + b.size.x);
    let y1 = (a.pos.y + a.size.y).min(b.pos.y + b.size.y);
    Rect { pos: dvec2(x0, y0), size: dvec2((x1 - x0).max(0.0), (y1 - y0).max(0.0)) }
}
```

- [ ] **Step 6: Rewrite `draw_relations_overlay` to draw veils**

Replace the whole body of `draw_relations_overlay` (the Task 2 calm-slate connector version) with:

```rust
    /// Persistent constraint overlay: each projected placement drawn as a
    /// distance-faded grey keep-out veil (spec §2). Visibility gating lands in
    /// Task 5; for now every relation is drawn.
    fn draw_relations_overlay(&mut self, cx: &mut Cx2d) {
        let legs: Vec<(usize, usize, waml::syntax::Direction)> = self
            .scene
            .relations
            .iter()
            .filter_map(|rel| {
                let si = self.scene.nodes.iter().position(|n| n.key == rel.subject)?;
                let ri = self.scene.nodes.iter().position(|n| n.key == rel.reference)?;
                Some((si, ri, rel.dir))
            })
            .collect();
        for (si, ri, dir) in legs {
            self.draw_veil_for(cx, si, ri, dir);
        }
    }
```

- [ ] **Step 7: Delete the connector helper and drag-overlay legs**

- Delete the entire `fn draw_relation_connector(...)` method.
- Delete the entire `fn relations_in_scope<'a>(...)` free function (~:472).
- In `draw_drag_overlay`, delete the relation-leg block: the `let legs: Vec<...> = relations_in_scope(...)...collect();` and its `for (si, ri, dir) in legs { ... self.draw_relation_connector(...) }` loop (the block that begins with the comment "Relation overlay + dock compass, centered on the target node"). KEEP the `let center = self.node_screen_center(ti); self.draw_compass(cx, center, self.compass_zone);` lines (move them out of the deleted block so the compass still draws). The surrounding `if let Some(ti) = self.drag_target { ... }` stays; only the relation legs are removed.
- Delete the canvas test `relations_in_scope_keeps_only_relations_touching_dragged_or_target` (the function it tests is gone).
- KEEP `dir_word` (the DSL readout `format!("{a_key} {} {b_key}", dir_word(d))` still uses it).

- [ ] **Step 8: Retire the debug group-bounds outline**

In `draw_walk`, inside the `for (screen, title) in group_draws` loop, delete the four `self.fill_rect(...)` outline bars (the block after the `// Debug outline: four thin slate bars hugging the rect border.` comment, including the `let ol = ...; let t = 1.5;`). KEEP `self.draw_group.draw_abs(cx, screen);` (the group fill) and the `if let Some(title) = &title { ... }` title draw.

- [ ] **Step 9: Run the tests**

Run: `cargo test -p waml-editor --lib canvas`
Expected: PASS — `veil_band_anchors_and_clamps_per_direction` passes; the deleted `relations_in_scope` test is gone; nothing references the removed symbols.

- [ ] **Step 10: Run the full gate**

Run: `cargo test --workspace`
Expected: PASS.
Run: `cargo clippy --workspace --all-targets -- -D warnings`
Expected: clean (no unused-import / dead-code warnings from the removals — remove any import that only served `draw_relation_connector`/`relations_in_scope`).

- [ ] **Step 11: Commit**

```bash
git add crates/waml-editor/src/canvas.rs
git commit -m "feat(canvas): hatched keep-out veil replaces connector notation"
```
