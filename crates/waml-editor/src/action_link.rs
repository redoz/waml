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
//! Interaction contract is copied verbatim from `RecentRowView` (the proven
//! `#[deref] View` hybrid): `handle_event` hit-tests its own area and fires on
//! `FingerUp`, FingerHoverIn/Out drives a `hovered` flag, and `draw_walk` pushes
//! that into the `hover` uniform on the root `draw_bg` wash before delegating.
//! The icon is a small `SolidView` child whose `draw_bg` shader is picked per
//! instance in the DSL (`IconNewProject` / `IconOpenProject`) -- the same
//! shader-per-glyph idiom as `icons.rs`, so no per-instance uniform plumbing.

use makepad_widgets::*;

script_mod! {
    use mod.prelude.widgets_internal.*
    use mod.atlas
    use mod.widgets.*
    use mod.text.*

    // "New project" glyph: a plus sign built from two crossing bars. Solid
    // `sdf.rect` fills ONLY (no `sdf.box` -- 0-radius floods this fork; no paths
    // -- they degenerate near edges). Geometry normalized against `rect_size`.
    // A `DrawQuad` subclass (like `LogoMark`) so it slots into a plain `View`'s
    // `draw_bg` (typed `DrawQuad`); the accent tint rides a `color` uniform.
    mod.draw.IconNewProject = mod.draw.DrawQuad{
        color: uniform(atlas.accent)
        pixel: fn() {
            let s = self.rect_size.x
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            // Horizontal bar.
            sdf.rect(s * 0.15, s * 0.43, s * 0.70, s * 0.14)
            sdf.fill(self.color)
            // Vertical bar.
            sdf.rect(s * 0.43, s * 0.15, s * 0.14, s * 0.70)
            sdf.fill(self.color)
            return sdf.result
        }
    }

    // "Open project" glyph: a folder = a body rect + a smaller tab rect riding
    // the top-left. Solid `sdf.rect` fills ONLY (same degeneracy rules as above).
    mod.draw.IconOpenProject = mod.draw.DrawQuad{
        color: uniform(atlas.accent)
        pixel: fn() {
            let s = self.rect_size.x
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            // Tab (top-left), sitting just above the body.
            sdf.rect(s * 0.12, s * 0.26, s * 0.34, s * 0.16)
            sdf.fill(self.color)
            // Folder body; overlaps the tab so they read as one shape.
            sdf.rect(s * 0.12, s * 0.38, s * 0.76, s * 0.44)
            sdf.fill(self.color)
            return sdf.result
        }
    }

    mod.widgets.ActionLinkBase = #(ActionLink::register_widget(vm))

    mod.widgets.ActionLink = set_type_default() do mod.widgets.ActionLinkBase{
        width: Fill
        height: Fit
        flow: Right
        align: Align{y: 0.5}
        padding: Inset{left: 8.0, right: 8.0, top: 6.0, bottom: 6.0}
        spacing: 10.0
        show_bg: true

        // Hover wash: the same subtle premultiplied accent fill as
        // `RecentRowView`, faded by the `hover` uniform (0 rest / 1 pointer-over)
        // the widget sets from FingerHoverIn/Out. Full-rect return (no `sdf.box`),
        // premultiplied so a low-alpha tint reads as a wash, not a bloom.
        draw_bg +: {
            color: atlas.accent
            hover: uniform(0.0)
            pixel: fn() {
                let a = 0.12 * self.hover
                return vec4(self.color.x * a, self.color.y * a, self.color.z * a, a)
            }
        }

        // Accent glyph. A fixed 16px `View` whose `draw_bg` shader is chosen per
        // instance (default = plus; the DSL instance overrides to the folder). A
        // plain `View` (not `SolidView`) so its `draw_bg` slot takes a `DrawColor`
        // icon shader by full assignment.
        icon := View {
            width: 16.0
            height: 16.0
            show_bg: true
            draw_bg: mod.draw.IconNewProject{ color: atlas.accent }
        }

        // Prose label. Set per instance via `label := { text: "..." }` (mirrors
        // how `RecentRowView` carries static text on child Labels).
        label := Label {
            text: ""
            draw_text +: {
                color: atlas.text
                text_style: theme.font_regular{font_size: 12 line_spacing: 1.0}
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
    /// The link row: the icon + prose label declared in the DSL tree above.
    #[deref]
    view: View,

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

    // Push the hover state into the wash uniform, then delegate the draw so the
    // icon + label render.
    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view
            .draw_bg
            .set_uniform(cx, live_id!(hover), &[if self.hovered { 1.0 } else { 0.0 }]);
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
