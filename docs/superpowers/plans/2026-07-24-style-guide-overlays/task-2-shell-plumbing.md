# Task 2: `OverlayShell` — draw/event plumbing (procedural seam)

**Files:**
- Modify: `crates/waml-editor/src/overlay_shell.rs`

**Interfaces:**
- Consumes (Task 1): `PanelGeom`, `EDGE_H`, the pad/scrollbar constants.
- Produces (consumed by Tasks 3–6):
  - `pub struct OverlayShell` — `#[derive(Live, LiveHook)]`, embedded via `#[live] shell: OverlayShell` on a consumer widget. `#[live]` fields: `draw_list: DrawList2d`, `draw_scrim: DrawColor`, `draw_panel: DrawColor`, `draw_edge: DrawColor`, `draw_thumb: DrawColor`, `panel_width: f64`. `#[rust]` state: `open: bool`, `scroll: f64`, `geom: PanelGeom`, `thumb_drag: Option<f64>`.
  - `pub struct OverlayShellPass { pub origin: DVec2, pub width: f64 }`
  - `pub enum OverlayShellAction { None, Dismissed }` (`#[derive(Clone, Debug, Default)]`, `None` is `#[default]`).
  - Methods: `is_open(&self) -> bool`, `set_open(&mut self, cx: &mut Cx, open: bool)`, `begin(&mut self, cx: &mut Cx2d, content_height: f64) -> Option<OverlayShellPass>`, `end(&mut self, cx: &mut Cx2d)`, `handle_event(&mut self, cx: &mut Cx, event: &Event) -> OverlayShellAction`.

**Seam contract (do NOT change to a callback):**
```rust
let h = self.content_height(cx);
if let Some(pass) = self.shell.begin(cx, h) {
    self.draw_rows(cx, pass.origin, pass.width);
    self.shell.end(cx);
}
```
`begin` returns `None` when closed (draws nothing). While open it draws the scrim + panel + top edge into the window overlay, snapshots geometry, and pushes the content clip. `end` pops the clip, draws the thumb, and closes the overlay list. A callback seam (`shell.draw(cx, &mut content)`) fails to borrow-check against `&mut self` — that is why this is procedural.

**Trap notes:**
- **dead_code:** no consumer lands until Task 3, so annotate the struct + inherent impl with `#[allow(dead_code)]` (the `LinearGeom` precedent). Do not delete the annotation; Task 3 makes it reachable but keeping it is harmless.
- **No `script_mod` for the shell.** `OverlayShell` is a nested `#[live]` struct (the `IconSet`-as-`#[live]`-field precedent in `popup/menu.rs`), not a widget — its `DrawColor` fields get their colors from the CONSUMER's DSL (`shell: { draw_scrim +: { color: atlas.scrim } }`). The plain `DrawColor` fill shader is correct for solid scrim/panel/edge/thumb rects.
- The Cx-bound `begin`/`end`/`handle_event` cannot run headless (no GPU `Cx`), so this unit's gate is compile + clippy + the Task 1 geometry tests staying green; the runtime behavior is visually verified in Task 3 (first consumer).

---

- [ ] **Step 1: Add the action + pass types**

At the top of `overlay_shell.rs` (below the `use`), add:

```rust
/// What `OverlayShell::handle_event` tells the owning widget to do.
#[derive(Clone, Debug, Default)]
pub enum OverlayShellAction {
    #[default]
    None,
    /// The scrim (outside the panel) was clicked — the owner should close.
    Dismissed,
}

/// Handed to the consumer by `begin`: where + how wide to draw its content.
pub struct OverlayShellPass {
    /// Top-left of the (already scroll-shifted, about-to-be-clipped) content.
    pub origin: DVec2,
    /// Content column width (panel width minus horizontal pad).
    pub width: f64,
}
```

- [ ] **Step 2: Add the struct**

Add (with `#[allow(dead_code)]`):

```rust
/// Shared overlay chrome embedded by each page widget. Owns its own window-
/// overlay draw list (`draw_list`) so it paints over the whole window,
/// including the caption band — the `MenuPopup`/`SelectFlyout` idiom. Open/close
/// is authoritative on the OWNER (App drives it for mutual exclusion); the
/// shell only tracks the `open` flag it is told, plus scroll + geometry.
#[allow(dead_code)]
#[derive(Live, LiveHook)]
pub struct OverlayShell {
    /// Window-overlay draw list (`begin_overlay_reuse`), same as `MenuPopup`.
    #[live]
    draw_list: DrawList2d,
    #[live]
    draw_scrim: DrawColor,
    #[live]
    draw_panel: DrawColor,
    /// Source-bright top edge hairline (shared HUD panel material).
    #[live]
    draw_edge: DrawColor,
    /// Scrollbar thumb.
    #[live]
    draw_thumb: DrawColor,
    /// Fixed panel width for this consumer (wired from its DSL).
    #[live]
    panel_width: f64,

    #[rust]
    open: bool,
    #[rust]
    scroll: f64,
    /// Snapshot of last-draw geometry, so `handle_event` (which runs between
    /// draws) can hit-test the panel/thumb and clamp scroll without a `Cx2d`.
    #[rust]
    geom: PanelGeom,
    /// Cursor y-offset from the thumb top while dragging it; `None` otherwise.
    #[rust]
    thumb_drag: Option<f64>,
}
```

- [ ] **Step 3: Implement open/close + the draw seam**

```rust
#[allow(dead_code)]
impl OverlayShell {
    pub fn is_open(&self) -> bool {
        self.open
    }

    /// Owner-driven visibility. `redraw_all` (not `draw_*.redraw`) because while
    /// closed nothing is ever drawn, so the draw areas stay `Area::Empty` and a
    /// targeted redraw is a no-op — the same reason `ShortcutsOverlay` used it.
    pub fn set_open(&mut self, cx: &mut Cx, open: bool) {
        if self.open != open {
            self.open = open;
            if !open {
                self.scroll = 0.0;
                self.thumb_drag = None;
            }
            cx.redraw_all();
        }
    }

    /// Begin drawing: scrim + panel + top edge into the window overlay, then push
    /// the content clip. Returns where the consumer should place its content, or
    /// `None` when closed (nothing drawn). Pair with `end`.
    pub fn begin(&mut self, cx: &mut Cx2d, content_height: f64) -> Option<OverlayShellPass> {
        if !self.open {
            return None;
        }
        let size = cx.current_pass_size();
        let mut geom = PanelGeom::new(size, self.panel_width, content_height);
        geom.set_scroll(self.scroll);
        self.scroll = geom.scroll();
        self.geom = geom;

        self.draw_list.begin_overlay_reuse(cx);
        cx.begin_root_turtle(size, Layout::flow_overlay());

        self.draw_scrim.draw_abs(cx, Rect { pos: dvec2(0.0, 0.0), size });
        self.draw_panel.draw_abs(cx, geom.panel_rect());
        self.draw_edge.draw_abs(cx, geom.edge_rect());

        cx.push_clip_rect(geom.clip_rect());
        Some(OverlayShellPass {
            origin: geom.content_origin(),
            width: geom.content_width(),
        })
    }

    /// End drawing: pop the content clip, draw the thumb over the (unclipped)
    /// rows, and close the overlay list. Only valid after a `begin` that
    /// returned `Some`.
    pub fn end(&mut self, cx: &mut Cx2d) {
        cx.pop_clip_rect();
        if let Some(thumb) = self.geom.thumb_rect() {
            self.draw_thumb.draw_abs(cx, thumb);
        }
        cx.end_pass_sized_turtle();
        self.draw_list.end(cx);
    }
}
```

- [ ] **Step 4: Implement `handle_event`**

Append to the same `impl OverlayShell`:

```rust
    /// Route a raw event while open: wheel-scroll + thumb-drag are consumed
    /// inside the panel; a primary press on the scrim (outside the panel)
    /// reports `Dismissed`; a press inside the panel is consumed (never
    /// dismisses). Escape is routed by the App (same path as the shortcuts
    /// overlay), not here, because a mounted overlay holds no key focus.
    pub fn handle_event(&mut self, cx: &mut Cx, event: &Event) -> OverlayShellAction {
        if !self.open {
            return OverlayShellAction::None;
        }
        match event {
            // Wheel over the panel scrolls; mark handled so the canvas below
            // doesn't also pan (the scroll-occlusion contract).
            Event::Scroll(e) if self.geom.panel_rect().contains(e.abs) => {
                let prev = self.scroll;
                self.geom.set_scroll(prev + e.scroll.y);
                self.scroll = self.geom.scroll();
                e.handled_x.set(true);
                e.handled_y.set(true);
                if self.scroll != prev {
                    cx.redraw_all();
                }
                OverlayShellAction::None
            }
            Event::MouseMove(e) => {
                if let Some(grab) = self.thumb_drag {
                    self.geom.set_scroll(self.geom.scroll_for_thumb_y(e.abs.y - grab));
                    self.scroll = self.geom.scroll();
                    cx.redraw_all();
                }
                OverlayShellAction::None
            }
            Event::MouseUp(e) if e.button.is_primary() => {
                self.thumb_drag = None;
                OverlayShellAction::None
            }
            Event::MouseDown(e) if e.button.is_primary() => {
                // Press on the thumb starts a drag; claim the press so the canvas
                // below can't capture the digit and pan (fork `hits()` bails
                // MouseDown when `handled` is non-empty).
                if let Some(thumb) = self.geom.thumb_rect() {
                    if thumb.contains(e.abs) {
                        self.thumb_drag = Some(e.abs.y - thumb.pos.y);
                        e.handled.set(self.draw_panel.area());
                        return OverlayShellAction::None;
                    }
                }
                if self.geom.panel_rect().contains(e.abs) {
                    // Inside the panel: consume (modal), never dismiss.
                    e.handled.set(self.draw_panel.area());
                    OverlayShellAction::None
                } else {
                    // On the scrim: dismiss. Claim the press so nothing below acts.
                    e.handled.set(self.draw_scrim.area());
                    OverlayShellAction::Dismissed
                }
            }
            _ => OverlayShellAction::None,
        }
    }
}
```

- [ ] **Step 5: Compile + clippy + geometry tests**

Run: `cargo test -p waml-editor --lib overlay_shell`
Expected: PASS (Task 1's 6 tests still green; new code compiles).

Run: `cargo clippy -p waml-editor -- -D warnings`
Expected: clean (the `#[allow(dead_code)]` covers the not-yet-consumed API).

- [ ] **Step 6: Commit**

```bash
git add crates/waml-editor/src/overlay_shell.rs
git commit -m "feat(overlay): OverlayShell draw/event plumbing via procedural begin/end seam"
```

Verification: full gate green. Runtime (scrim/panel paints over caption band, wheel + thumb scroll, scrim-click dismiss) is visually verified in Task 3, the first consumer — the headless gate cannot boot the script VM.
