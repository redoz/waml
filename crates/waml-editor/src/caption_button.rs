//! `CaptionButton`: a borderless square icon button for the right edge of the
//! window caption bar (`app.rs`'s `caption_bar` tree). Two instances ship: a
//! save glyph and a hamburger-menu glyph. Both are PLACEHOLDERS this pass -- a
//! primary click emits `CaptionButtonAction::Clicked`, which `App` maps to a
//! `log!` line; real save / real menu land later.
//!
//! Interaction contract is copied from `ActionLink` (the proven `#[deref] View`
//! hybrid): `handle_event` hit-tests its own `view.area()` and fires on
//! `FingerUp`, `FingerHoverIn/Out` drives a `hovered` flag + the Hand cursor,
//! and `draw_walk` pushes state into the root `draw_bg` uniforms before
//! delegating. Idle stroke = `atlas.text` (full weight, not dimmed -- a dim
//! idle reads as disabled); hover tints to `atlas.accent` plus a rounded-square
//! accent highlight behind the glyph.
//!
//! The glyph is selected by the widget's OWN `#[live] shape` scalar prop
//! (`0.0 = hamburger`, `1.0 = save`), NOT a DSL instance child-override: this
//! fork's `script_mod` DSL has no child-override on an instance. So both glyphs
//! are folded into the root shader and the unwanted one is pushed off-screen by
//! `shape` (a branchless select -- an `if` on a uniform silently no-ops in this
//! fork's `draw_bg` pixel VM, same as `ActionLink`).
//!
//! Glyph geometry is a faithful port of the Lucide `menu.svg` / `save.svg`
//! centerlines -- the same path commands the shared `IconMenu` / `IconSave`
//! shaders use (icons.rs), remapped into a `gs`-sized frame centered in the
//! caption bar rather than a full-bleed icon rect.
//!
//! The caption bar is an OS window-drag region, so `App::handle_event`
//! re-answers the `WindowDragQuery` as `Client` over each button's rect (the
//! same seam that keeps the doc tabs clickable). `hits` exposes the last-drawn
//! rect for that query.

use makepad_widgets::*;

script_mod! {
    use mod.prelude.widgets_internal.*
    use mod.atlas
    use mod.widgets.*

    mod.widgets.CaptionButtonBase = #(CaptionButton::register_widget(vm))

    mod.widgets.CaptionButton = set_type_default() do mod.widgets.CaptionButtonBase{
        // 30x44 hit box spanning the caption bar's full height (a Fill height
        // collapses to 0 in the Fit-height bar), so the shader can center both
        // the hover highlight and the glyph on the box middle.
        width: 36.0
        height: 44.0
        margin: Inset{left: 3.0, right: 3.0}
        show_bg: true

        // Root shader: a subtle premultiplied accent hover wash PLUS one of two
        // glyphs selected by `shape` (0 = hamburger, 1 = save). Both glyphs are
        // drawn every frame; the unwanted one is shoved off-screen in x by a
        // `shape`-gated offset (branchless, since `if` on a uniform no-ops in
        // this fork's draw_bg VM). Stroke tint mixes text_dim -> accent by
        // `hover`. `sdf.rect` for sharp edges (0-radius `sdf.box` floods this
        // fork). Colors are chosen INSIDE the shader from atlas tokens -- no
        // RGBA crosses Rust.
        draw_bg +: {
            text_col: uniform(atlas.text)
            accent_col: uniform(atlas.accent)
            sel_col: uniform(atlas.selection)
            hover: uniform(0.0)
            shape: uniform(0.0)
            pixel: fn() {
                let p = self.pos * self.rect_size
                let sdf = Sdf2d.viewport(p)
                let col = mix(self.text_col, self.accent_col, self.hover)
                // A `gs`-sized square glyph frame centered in the box; `oy` is its
                // top edge. Each glyph is a faithful port of its Lucide SVG (see
                // `IconSave`/`IconMenu` in icons.rs), whose path coords are
                // fractions of the frame side `gs`. `w` matches the icon set's
                // 0.068 stroke ratio.
                let cx = self.rect_size.x * 0.5
                // +1px downward bias: geometric center (rect_size.y*0.5) reads
                // optically high next to the lower-sitting doc-tab text; nudge
                // the whole glyph frame + hover box down a hair.
                let cy = self.rect_size.y * 0.5 + 1.0
                let gs = 18.0
                let oy = cy - gs * 0.5
                // 1.2px: a touch under the fork's hidpi coverage floor (~1.5),
                // so lines read fine/light. Much thinner (1.0-) fades further.
                let w = 1.2
                let w_save = 1.2

                // Hover highlight: a rounded square behind the glyph, painted
                // first so the glyph sits on top. The SAME accent-tint token the
                // app-menu rows use (`atlas.selection`, blue @ ~13% straight
                // alpha), faded in by `hover` (0 => fully transparent when idle).
                // Straight alpha: this fork's `sdf.fill` blends straight, so a
                // premult `accent*a` fill would double-apply `a` and wash the
                // tint out to a neutral grey.
                let side = 32.0
                sdf.box(cx - side * 0.5, cy - side * 0.5, side, side, 2.0)
                sdf.fill(vec4(self.sel_col.x, self.sel_col.y, self.sel_col.z, self.sel_col.w * self.hover))

                // Branchless glyph select (an `if` on a uniform no-ops in this
                // fork's draw_bg VM): fold a far off-screen shift into each
                // glyph's x-origin, so the unwanted glyph -- arc centers and all
                // -- leaves the viewport and only the chosen one samples.
                let ox_m = cx - gs * 0.5 + self.shape * 9999.0
                let ox_s = cx - gs * 0.5 + (1.0 - self.shape) * 9999.0

                // --- hamburger (menu.svg / IconMenu): three bars ---
                sdf.move_to(ox_m + gs * 0.1667, oy + gs * 0.2083)
                sdf.line_to(ox_m + gs * 0.8333, oy + gs * 0.2083)
                sdf.stroke(col, w)
                sdf.move_to(ox_m + gs * 0.1667, oy + gs * 0.5000)
                sdf.line_to(ox_m + gs * 0.8333, oy + gs * 0.5000)
                sdf.stroke(col, w)
                sdf.move_to(ox_m + gs * 0.1667, oy + gs * 0.7917)
                sdf.line_to(ox_m + gs * 0.8333, oy + gs * 0.7917)
                sdf.stroke(col, w)

                // --- save (save.svg / IconSave): floppy body + label + shutter ---
                sdf.move_to(ox_s + gs * 0.6333, oy + gs * 0.1250)
                sdf.arc_to(ox_s + gs * 0.6321, oy + gs * 0.2083, gs * 0.0833, -1.5566, -0.7753)
                sdf.line_to(ox_s + gs * 0.8500, oy + gs * 0.3083)
                sdf.arc_to(ox_s + gs * 0.7917, oy + gs * 0.3679, gs * 0.0833, -0.7955, -0.0142)
                sdf.line_to(ox_s + gs * 0.8750, oy + gs * 0.7917)
                sdf.arc_to(ox_s + gs * 0.7917, oy + gs * 0.7917, gs * 0.0833, 0.0000, 1.5708)
                sdf.line_to(ox_s + gs * 0.2083, oy + gs * 0.8750)
                sdf.arc_to(ox_s + gs * 0.2083, oy + gs * 0.7917, gs * 0.0833, 1.5708, 3.1416)
                sdf.line_to(ox_s + gs * 0.1250, oy + gs * 0.2083)
                sdf.arc_to(ox_s + gs * 0.2083, oy + gs * 0.2083, gs * 0.0833, 3.1416, 4.7124)
                sdf.close_path()
                sdf.stroke(col, w_save)
                sdf.move_to(ox_s + gs * 0.7083, oy + gs * 0.8750)
                sdf.line_to(ox_s + gs * 0.7083, oy + gs * 0.5833)
                sdf.arc_to(ox_s + gs * 0.6667, oy + gs * 0.5833, gs * 0.0417, 0.0000, -1.5708)
                sdf.line_to(ox_s + gs * 0.3333, oy + gs * 0.5417)
                sdf.arc_to(ox_s + gs * 0.3333, oy + gs * 0.5833, gs * 0.0417, -1.5708, -3.1416)
                sdf.line_to(ox_s + gs * 0.2917, oy + gs * 0.8750)
                sdf.stroke(col, w_save)
                sdf.move_to(ox_s + gs * 0.2917, oy + gs * 0.1250)
                sdf.line_to(ox_s + gs * 0.2917, oy + gs * 0.2917)
                sdf.arc_to(ox_s + gs * 0.3333, oy + gs * 0.2917, gs * 0.0417, 3.1416, 1.5708)
                sdf.line_to(ox_s + gs * 0.6250, oy + gs * 0.3333)
                sdf.stroke(col, w_save)

                return sdf.result
            }
        }
    }
}

/// Button input, read by `App::handle_actions`. `Clicked` fires on a primary
/// release over the button (the save glyph's model). `Pressed` fires on the
/// primary press and carries the press position, so a menu button can open its
/// drop-down on the DOWN edge and hand the press point to a marking drag (the
/// burger's model).
#[derive(Clone, Debug, Default)]
pub enum CaptionButtonAction {
    #[default]
    None,
    Clicked,
    Pressed(DVec2),
}

#[derive(Script, ScriptHook, Widget)]
pub struct CaptionButton {
    /// The button box: the folded-glyph root `draw_bg` declared above.
    #[deref]
    view: View,

    /// Glyph selector fed to the root shader's `shape` uniform: 0 = hamburger,
    /// 1 = save. Set per DSL instance as a root scalar prop.
    #[live]
    shape: f32,

    /// Pointer-over state, self-managed from FingerHoverIn/Out; feeds the
    /// `hover` uniform (stroke tint + wash) each `draw_walk`.
    #[rust]
    hovered: bool,

    /// Held-open state, driven by `App`: true while this button's drop-down menu
    /// is open, so the glyph keeps its active (accent) look even as the pointer
    /// leaves to travel down the menu. OR'd into the `hover` uniform.
    #[rust]
    held: bool,

    /// Last-drawn absolute rect, cached in `draw_walk` so the window drag-query
    /// can treat this button as `Client` area without a `cx` (mirrors the way
    /// `DocTabs` caches its tab rects).
    #[rust]
    rect: Rect,
}

impl Widget for CaptionButton {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, _scope: &mut Scope) {
        let uid = self.widget_uid();
        match event.hits(cx, self.view.area()) {
            Hit::FingerDown(fe) if fe.is_primary_hit() => {
                cx.widget_action(uid, CaptionButtonAction::Pressed(fe.abs));
            }
            Hit::FingerUp(fe) if fe.is_primary_hit() && fe.is_over => {
                cx.widget_action(uid, CaptionButtonAction::Clicked);
            }
            Hit::FingerHoverIn(_) => {
                cx.set_cursor(MouseCursor::Hand);
                self.hovered = true;
                self.view.redraw(cx);
            }
            Hit::FingerHoverOut(_) => {
                self.hovered = false;
                self.view.redraw(cx);
            }
            _ => {}
        }
    }

    // Push hover/shape uniforms, delegate, then cache the drawn rect for the
    // drag-query seam.
    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_bg.set_uniform(
            cx,
            live_id!(hover),
            &[if self.hovered || self.held { 1.0 } else { 0.0 }],
        );
        self.view.draw_bg.set_uniform(cx, live_id!(shape), &[self.shape]);
        let step = self.view.draw_walk(cx, scope, walk);
        self.rect = self.view.area().rect(cx);
        step
    }
}

impl CaptionButton {
    /// True when this button emitted a click in `actions`.
    pub fn clicked(&self, actions: &Actions) -> bool {
        actions
            .find_widget_action(self.widget_uid())
            .is_some_and(|a| matches!(a.cast(), CaptionButtonAction::Clicked))
    }

    /// The press position when this button emitted a primary press in
    /// `actions`, else `None`. Used to open a menu on the DOWN edge.
    pub fn pressed(&self, actions: &Actions) -> Option<DVec2> {
        actions
            .find_widget_action(self.widget_uid())
            .and_then(|a| match a.cast() {
                CaptionButtonAction::Pressed(p) => Some(p),
                _ => None,
            })
    }

    /// Whether `abs` lands on this button's last-drawn rect. Used by the window
    /// drag-query so the button is client area, not OS-draggable caption.
    pub fn hits(&self, abs: DVec2) -> bool {
        self.rect.contains(abs)
    }

    /// The button's last-drawn absolute rect (menu anchor: the burger drops a
    /// drop-down from its bottom-left).
    pub fn rect(&self) -> Rect {
        self.rect
    }

    /// Drive the held-open state (see [`CaptionButton::held`]). Redraws only on
    /// a change so `App` can call it every event cheaply.
    pub fn set_held(&mut self, cx: &mut Cx, held: bool) {
        if self.held != held {
            self.held = held;
            self.view.redraw(cx);
        }
    }
}

impl CaptionButtonRef {
    /// See [`CaptionButton::clicked`].
    pub fn clicked(&self, actions: &Actions) -> bool {
        self.borrow().is_some_and(|inner| inner.clicked(actions))
    }

    /// See [`CaptionButton::pressed`].
    pub fn pressed(&self, actions: &Actions) -> Option<DVec2> {
        self.borrow().and_then(|inner| inner.pressed(actions))
    }

    /// See [`CaptionButton::hits`].
    pub fn hits(&self, abs: DVec2) -> bool {
        self.borrow().is_some_and(|inner| inner.hits(abs))
    }

    /// See [`CaptionButton::rect`].
    pub fn rect(&self) -> Rect {
        self.borrow().map(|inner| inner.rect()).unwrap_or_default()
    }

    /// See [`CaptionButton::set_held`].
    pub fn set_held(&self, cx: &mut Cx, held: bool) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.set_held(cx, held);
        }
    }
}
