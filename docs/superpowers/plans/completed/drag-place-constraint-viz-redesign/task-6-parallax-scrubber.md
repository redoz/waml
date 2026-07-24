### Task 6: Parallax layer scrubber

**Files:**
- Modify: `crates/waml-editor/src/canvas.rs` (pure `parallax_offset`; `scrub_layer`/`parallax_base` state; per-layer parallax in All-mode veil draw; `cycle_scrub_layer`; keyboard scrub)

**Interfaces:**
- Consumes (from Task 5): `ConstraintVisibility::All`, `relations_for_visibility`, `draw_veil_for`.
- Produces: `parallax_offset(pan_delta: DVec2, layer_depth: i32, spread: f64) -> DVec2` (pure); `GraphCanvas::cycle_scrub_layer(&mut self, cx, delta: i32)`.

**Design (spec §3):** In All mode each constraint gets its own layer (one relation = one layer). Layers shift at different rates as the view pans, so overlapping veils separate by *motion* (no 3D camera). `scrub_layer` selects the front layer (depth 0); other layers are offset by `parallax_offset(pan_delta, i - scrub_layer, SPREAD)`. `pan_delta` is the camera pan drift since the scene fitted. Selected mode (few layers) simply stacks with the same parallax. A polished on-screen scrubber control is an interactive-signoff-deferred nicety (see README); the testable core is the offset math + per-layer application, and keyboard `[`/`]` advances the front layer.

---

- [ ] **Step 1: Write the failing `parallax_offset` test**

Add to `canvas.rs`'s `#[cfg(test)] mod tests`:

```rust
#[test]
fn parallax_offset_scales_with_depth_and_pan() {
    // The front layer (depth 0) never shifts.
    assert_eq!(parallax_offset(dvec2(100.0, -50.0), 0, 0.1), dvec2(0.0, 0.0));
    // Depth scales the pan drift linearly; sign follows the pan.
    assert_eq!(parallax_offset(dvec2(100.0, -50.0), 2, 0.1), dvec2(20.0, -10.0));
    // Negative depth shifts the opposite way (layers behind vs in front).
    assert_eq!(parallax_offset(dvec2(100.0, -50.0), -1, 0.1), dvec2(-10.0, 5.0));
    // Deeper layers shift monotonically further for a fixed pan.
    let d1 = parallax_offset(dvec2(80.0, 0.0), 1, 0.2).x;
    let d3 = parallax_offset(dvec2(80.0, 0.0), 3, 0.2).x;
    assert!(d3.abs() > d1.abs());
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p waml-editor --lib canvas::tests::parallax_offset`
Expected: FAIL to compile — `parallax_offset` undefined.

- [ ] **Step 3: Add the pure `parallax_offset` + a spread constant**

Add to `canvas.rs` near the other pure helpers:

```rust
/// How strongly a layer's depth multiplies view-pan drift into a parallax shift
/// (spec §3). Small so panning gently separates stacked veils.
const PARALLAX_SPREAD: f64 = 0.14;

/// Per-layer parallax shift: the pan drift times the layer's signed depth times
/// `spread`. Depth 0 (the front / scrubbed layer) never moves; deeper/nearer
/// layers slide at proportional rates so overlapping veils separate by motion.
/// Pure, GPU-free.
fn parallax_offset(pan_delta: DVec2, layer_depth: i32, spread: f64) -> DVec2 {
    let k = layer_depth as f64 * spread;
    dvec2(pan_delta.x * k, pan_delta.y * k)
}
```

- [ ] **Step 4: Add scrub state + parallax base capture**

Add fields to `GraphCanvas` (near `constraint_vis`):

```rust
    /// Front layer index in All mode (spec §3); other layers parallax-offset by
    /// their depth relative to this. Clamped to the relation count.
    #[rust]
    scrub_layer: usize,
    /// Camera pan captured when the scene fitted, so All-mode parallax measures
    /// pan DRIFT (not absolute pan). `None` until first fit.
    #[rust]
    parallax_base: Option<(f64, f64)>,
```

In `draw_walk`, right after the `self.fitted = true;` line inside the fit block, capture the base:

```rust
                self.fitted = true;
                self.parallax_base = Some((self.camera.pan_x, self.camera.pan_y));
```

Reset it in `set_scene`/`set_focus` alongside the other resets (add `self.parallax_base = None; self.scrub_layer = 0;` where `self.fitted = false;` is set in both).

- [ ] **Step 5: Apply parallax to the All-mode veil draw**

Change `draw_veil_for` to accept a screen offset (add the param; apply it to the band position and every scrim rect). Update its signature and the two internal draws:

```rust
    fn draw_veil_for(
        &mut self,
        cx: &mut Cx2d,
        subject_idx: usize,
        reference_idx: usize,
        dir: waml::syntax::Direction,
        offset: DVec2,
    ) {
        let mut reference_screen = self.node_screen_rect(reference_idx);
        reference_screen.pos += offset;
        let band = veil_band(reference_screen, self.view_rect, dir, VEIL_REACH);
        // ... unchanged clip/guard/uniforms/draw_abs(band) ...
```

and in the scrim loop, add `offset` to each card's screen pos:

```rust
            if let Some(i) = self.scene.nodes.iter().position(|n| n.key == key) {
                let mut s = self.node_screen_rect(i);
                s.pos += offset;
                self.fill_rect(cx, s.pos.x, s.pos.y, s.size.x, s.size.y, vec4(0.62, 0.65, 0.70, 0.45));
            }
```

Rewrite `draw_relations_overlay` to feed the offset — zero for None/Selected, parallax per layer for All:

```rust
    fn draw_relations_overlay(&mut self, cx: &mut Cx2d) {
        let selected_key = self.selected_key.clone();
        let chosen: Vec<(usize, usize, waml::syntax::Direction)> =
            relations_for_visibility(&self.scene.relations, self.constraint_vis, selected_key.as_deref())
                .into_iter()
                .filter_map(|rel| {
                    let si = self.scene.nodes.iter().position(|n| n.key == rel.subject)?;
                    let ri = self.scene.nodes.iter().position(|n| n.key == rel.reference)?;
                    Some((si, ri, rel.dir))
                })
                .collect();

        let parallax = if self.constraint_vis == ConstraintVisibility::All {
            let base = self.parallax_base.unwrap_or((self.camera.pan_x, self.camera.pan_y));
            // World pan drift → screen drift (pan is in world units; scale by zoom).
            let dx = (self.camera.pan_x - base.0) * self.camera.zoom;
            let dy = (self.camera.pan_y - base.1) * self.camera.zoom;
            Some(dvec2(dx, dy))
        } else {
            None
        };

        for (layer, (si, ri, dir)) in chosen.into_iter().enumerate() {
            let offset = match parallax {
                Some(pan_delta) => parallax_offset(pan_delta, layer as i32 - self.scrub_layer as i32, PARALLAX_SPREAD),
                None => dvec2(0.0, 0.0),
            };
            self.draw_veil_for(cx, si, ri, dir, offset);
        }
    }
```

- [ ] **Step 6: Add `cycle_scrub_layer` + keyboard scrub**

Add the public method to `impl GraphCanvas`:

```rust
    /// Advance the front scrub layer in All mode, clamped to the relation count.
    pub fn cycle_scrub_layer(&mut self, cx: &mut Cx, delta: i32) {
        let n = self.scene.relations.len();
        if n == 0 {
            self.scrub_layer = 0;
        } else {
            let max = n - 1;
            let next = (self.scrub_layer as i32 + delta).clamp(0, max as i32);
            self.scrub_layer = next as usize;
        }
        self.draw_bg.redraw(cx);
    }
```

In the canvas `handle_event`, add key handling for `[`/`]` at the top of the method (only acts in All mode):

```rust
        if let Event::KeyDown(ke) = event {
            if self.constraint_vis == ConstraintVisibility::All {
                match ke.key_code {
                    KeyCode::BracketRight => { self.cycle_scrub_layer(cx, 1); return; }
                    KeyCode::BracketLeft => { self.cycle_scrub_layer(cx, -1); return; }
                    _ => {}
                }
            }
        }
```

(If key events do not reach the canvas without key focus, the parallax-by-pan separation still satisfies this task; the `[`/`]` scrub is validated at interactive sign-off — see README. The gate here is the pure `parallax_offset` test + build.)

- [ ] **Step 7: Run the tests**

Run: `cargo test -p waml-editor --lib canvas`
Expected: PASS (`parallax_offset_scales_with_depth_and_pan` plus the Task 4/5 canvas tests).

- [ ] **Step 8: Run the full gate**

Run: `cargo test --workspace`
Expected: PASS.
Run: `cargo clippy --workspace --all-targets -- -D warnings`
Expected: clean.

- [ ] **Step 9: Commit**

```bash
git add crates/waml-editor/src/canvas.rs
git commit -m "feat(canvas): parallax layer separation + scrub for All-mode veils"
```
