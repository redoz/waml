//! `ActionLink`: a Visual-Studio-style borderless action link for the start
//! screen's START column -- `[accent icon] [gap] [prose label]`, no button
//! chrome. Hover paints a subtle premultiplied accent wash behind the whole row
//! (same material as `RecentRowView`) and switches to the Hand cursor; a primary
//! click over the row emits `ActionLinkAction::Clicked`, which
//! `StartScreen::handle_actions` maps to a `StartScreenAction`.
//!
//! This retires the old bordered HUD action button: the start screen was its
//! only consumer, and the VS "Get started" look (icon + descriptive text link)
//! reads lighter than a filled button for the launcher's two actions.
//!
//! Interaction contract is copied from `RecentRowView` (the proven `#[deref]
//! View` hybrid): `handle_event` hit-tests its own area and fires on `FingerUp`,
//! FingerHoverIn/Out drives a `hovered` flag, and `draw_walk` pushes state into
//! the root `draw_bg` uniforms before delegating.
//!
//! The label + glyph are selected by the widget's OWN `#[live]` scalar props
//! (`text`, `kind`), NOT by DSL instance overrides: this fork's `script_mod`
//! DSL has no child-override on an instance (`icon = {..}` errors "variable not
//! found"; `icon := {..}` replaces the child with an untyped object that never
//! renders). So the icon is folded into the root shader (plus when `kind` < 0.5,
//! folder otherwise) and the label text is pushed onto the child `Label` from
//! `self.text` each `draw_walk` -- both settable per instance as root props (the
//! same way the retired `WamlButton` carried a `#[live] text`).

use makepad_widgets::*;

script_mod! {
    use mod.prelude.widgets_internal.*
    use mod.atlas
    use mod.widgets.*
    use mod.text.*
    use mod.fonts

    mod.widgets.ActionLinkBase = #(ActionLink::register_widget(vm))

    mod.widgets.ActionLink = set_type_default() do mod.widgets.ActionLinkBase{
        width: Fill
        height: Fit
        flow: Right
        align: Align{y: 0.5}
        // Left padding clears the 16px icon the root shader draws at x=8.
        padding: Inset{left: 32.0, right: 8.0, top: 3.0, bottom: 3.0}
        spacing: 0.0
        show_bg: true

        // Root shader: a subtle premultiplied hover wash (faded by the `hover`
        // uniform) PLUS the accent glyph at the left -- a plus when `kind` < 0.5,
        // a folder otherwise. Folding the icon in here sidesteps per-child uniform
        // plumbing (no instance child-override in this DSL). `sdf.rect` ONLY
        // (0-radius `sdf.box` floods this fork); drawn in PIXEL space so the
        // square glyph isn't stretched by the wide row. Premultiplied like
        // `CardShadow`/`RecentRowView` so a low-alpha wash reads as a wash.
        draw_bg +: {
            color: atlas.accent
            hover: uniform(0.0)
            kind: uniform(0.0)
            pixel: fn() {
                let p = self.pos * self.rect_size
                let ix = 8.0
                let iy = self.rect_size.y * 0.5 - 8.0
                let s = 16.0
                let sdf = Sdf2d.viewport(p)
                // Branchless glyph select: an `if` on the uniform silently no-ops
                // in this fork's shader VM, so draw BOTH glyphs and push the
                // unwanted one far off-screen in x (step gate) -- the visible icon
                // box only ever samples one. `kind` < 0.5 = plus, else folder.
                let plus_off = self.kind * 9999.0
                let fold_off = (1.0 - self.kind) * 9999.0
                // Plus: horizontal + vertical bar.
                sdf.rect(ix + s * 0.15 + plus_off, iy + s * 0.43, s * 0.70, s * 0.14)
                sdf.fill(self.color)
                sdf.rect(ix + s * 0.43 + plus_off, iy + s * 0.15, s * 0.14, s * 0.70)
                sdf.fill(self.color)
                // Folder: top-left tab + body.
                sdf.rect(ix + s * 0.12 + fold_off, iy + s * 0.26, s * 0.34, s * 0.16)
                sdf.fill(self.color)
                sdf.rect(ix + s * 0.12 + fold_off, iy + s * 0.38, s * 0.76, s * 0.44)
                sdf.fill(self.color)
                let icon = sdf.result
                let a = 0.12 * self.hover
                let wash = vec4(self.color.x * a, self.color.y * a, self.color.z * a, a)
                // Icon over wash (icon.w is its coverage).
                return icon + wash * (1.0 - icon.w)
            }
        }

        // Prose label. Text is pushed from `self.text` in `draw_walk` (the DSL
        // instance sets the widget's `text:` prop, not this child directly).
        label := Label {
            text: ""
            draw_text +: {
                color: atlas.text
                text_style: fonts.text_label
            }
        }
    }
}

/// Emitted when the link is clicked (FingerUp over its own area). Read by
/// `StartScreen::handle_actions` via `ActionLinkRef::clicked`.
#[derive(Clone, Debug, Default)]
pub enum ActionLinkAction {
    #[default]
    None,
    Clicked,
}

#[derive(Script, ScriptHook, Widget)]
pub struct ActionLink {
    /// The link row: the folded-icon root `draw_bg` + prose label declared in
    /// the DSL tree above.
    #[deref]
    view: View,

    /// Prose label, set per instance via the DSL `text:` prop; pushed onto the
    /// child `Label` each `draw_walk`.
    #[live]
    text: String,
    /// Icon selector fed to the root shader's `kind` uniform: 0 = plus (new),
    /// 1 = folder (open).
    #[live]
    kind: f32,

    /// Pointer-over state, self-managed from FingerHoverIn/Out; fed to the
    /// `hover` uniform on the root `draw_bg` each `draw_walk` for the wash.
    #[rust]
    hovered: bool,
}

impl Widget for ActionLink {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, _scope: &mut Scope) {
        let uid = self.widget_uid();
        match event.hits(cx, self.view.area()) {
            Hit::FingerUp(fe) if fe.is_primary_hit() && fe.is_over => {
                cx.widget_action(uid, ActionLinkAction::Clicked);
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

    // Push label text + hover/kind uniforms, then delegate so the label renders
    // over the folded-icon root shader.
    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.label(cx, ids!(label)).set_text(cx, &self.text);
        self.view
            .draw_bg
            .set_uniform(cx, live_id!(hover), &[if self.hovered { 1.0 } else { 0.0 }]);
        self.view
            .draw_bg
            .set_uniform(cx, live_id!(kind), &[self.kind]);
        self.view.draw_walk(cx, scope, walk)
    }
}

impl ActionLink {
    /// True when this link emitted a click in `actions`.
    pub fn clicked(&self, actions: &Actions) -> bool {
        actions
            .find_widget_action(self.widget_uid())
            .is_some_and(|a| matches!(a.cast(), ActionLinkAction::Clicked))
    }
}

impl ActionLinkRef {
    /// See [`ActionLink::clicked`].
    pub fn clicked(&self, actions: &Actions) -> bool {
        self.borrow().is_some_and(|inner| inner.clicked(actions))
    }
}
