//! `RefCardView`: one inspector reference card -- a compact, SQUARE-cornered
//! bordered row backing both MEMBERS and ASSOCIATIONS. Line 1 = element-kind
//! lead icon + name; line 2 (optional, dim) = a meta run (associations show the
//! direction glyph + role + multiplicity; members omit it). A `#[deref] View`
//! hybrid mirroring `recent_row.rs`; clicking a card emits `RefCardViewAction`
//! so the parent (`Inspector::navigate`) can repoint + select the target.
//! Values (including the nav target) are pushed per row by the parent's
//! FlatList loop.
//!
//! The border is drawn with `sdf.rect` for sharp square corners -- NEVER
//! `sdf.box(.., 0.0)`, which degenerates and floods in this fork. The lead glyph
//! is drawn in `draw_walk` over the reserved `icon_slot` gutter, the same
//! immediate-over-turtle idiom `select_box.rs` uses for its caret.

use crate::icons::{Icon, IconSet};
use crate::inspector::ElementKind;
use makepad_widgets::*;

script_mod! {
    use mod.prelude.widgets_internal.*
    use mod.atlas
    use mod.widgets.*
    use mod.text.*

    mod.widgets.RefCardViewBase = #(RefCardView::register_widget(vm))

    mod.widgets.RefCardView = set_type_default() do mod.widgets.RefCardViewBase{
        width: Fill
        height: Fit
        flow: Right
        align: Align{y: 0.5}
        padding: Inset{left: 8.0, right: 8.0, top: 6.0, bottom: 6.0}
        spacing: 8.0
        show_bg: true

        // Square-cornered card: faint field-bg fill + low-alpha accent ring,
        // both nudged up by the `hover` uniform (0 rest / 1 pointer-over) the
        // widget sets from FingerHoverIn/Out. `sdf.rect` (NOT `sdf.box(..,0)`,
        // which floods this fork).
        draw_bg +: {
            color: atlas.field_bg
            border: uniform(atlas.accent)
            hover: uniform(0.0)
            pixel: fn() {
                let sdf = Sdf2d.viewport(self.pos * self.rect_size)
                sdf.rect(0.5, 0.5, self.rect_size.x - 1.0, self.rect_size.y - 1.0)
                sdf.fill_keep(vec4(self.color.x, self.color.y, self.color.z, 0.5 + 0.15 * self.hover))
                sdf.stroke(vec4(self.border.x, self.border.y, self.border.z, 0.20 + 0.25 * self.hover), 1.0)
                return sdf.result
            }
        }

        // Reserved leading gutter; the lead glyph is drawn over this rect in
        // draw_walk (an 18-unit icon centered in a 18-wide slot).
        icon_slot := View {
            width: 18.0
            height: 18.0
        }

        textcol := View {
            width: Fill
            height: Fit
            flow: Down
            spacing: 1.0

            name := Label {
                width: Fill
                text: ""
                draw_text +: {
                    color: atlas.text
                    text_style: TextStyle{
                        font_size: 13
                        font_family: FontFamily{
                            latin := FontMember{res: crate_resource("self:resources/fonts/IBM_Plex_Sans/IBMPlexSans-Regular.ttf") asc: -0.1 desc: 0.0}
                        }
                        line_spacing: 1.1
                    }
                }
            }
            meta := Label {
                text: ""
                draw_text +: {
                    color: atlas.text_dim
                    text_style: TextStyle{
                        font_size: 11
                        font_family: FontFamily{
                            latin := FontMember{res: crate_resource("self:resources/fonts/IBM_Plex_Sans/IBMPlexSans-Regular.ttf") asc: -0.1 desc: 0.0}
                        }
                        line_spacing: 1.1
                    }
                }
            }
        }

        // Icon tint holder: an atlas-token DrawColor whose `.color` is copied
        // into `IconSet::draw` (no RGBA literal crosses Rust; see icons.rs).
        draw_icon: mod.draw.DrawColor{ color: atlas.text }
    }
}

/// Emitted (grouped through the parent FlatList) when a card is clicked. The
/// parent reads it via `items_with_actions` + `RefCardViewRef::nav_target`.
#[derive(Clone, Debug, Default)]
pub enum RefCardViewAction {
    #[default]
    None,
    Clicked,
}

#[derive(Script, ScriptHook, Widget)]
pub struct RefCardView {
    #[deref]
    view: View,
    #[live]
    icons: IconSet,
    #[redraw]
    #[live]
    draw_icon: DrawColor,
    /// The lead glyph for this row's element kind; `None` draws no icon.
    #[rust]
    icon: Option<Icon>,
    /// The navigate target this row points at (set per draw by the parent).
    #[rust]
    nav_key: String,
    #[rust]
    nav_kind: Option<ElementKind>,
    /// Pointer-over, self-managed from FingerHoverIn/Out (drives the cursor).
    #[rust]
    hovered: bool,
}

impl Widget for RefCardView {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);
        let uid = self.widget_uid();
        match event.hits(cx, self.view.area()) {
            Hit::FingerUp(fe) if fe.is_primary_hit() && fe.is_over => {
                cx.widget_action(uid, RefCardViewAction::Clicked);
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

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view
            .draw_bg
            .set_uniform(cx, live_id!(hover), &[if self.hovered { 1.0 } else { 0.0 }]);
        while self.view.draw_walk(cx, scope, walk).step().is_some() {}
        // Draw the lead glyph over the reserved slot's drawn rect.
        if let Some(icon) = self.icon {
            let slot = self.view.view(cx, ids!(icon_slot)).area().rect(cx);
            let tint = self.draw_icon.color;
            self.icons.draw(cx, icon, slot, tint);
        }
        DrawStep::done()
    }
}

impl RefCardView {
    pub fn set_icon(&mut self, cx: &mut Cx, icon: Icon) {
        self.icon = Some(icon);
        self.view.redraw(cx);
    }
    pub fn set_name(&mut self, cx: &mut Cx, s: &str) {
        self.view.label(cx, ids!(textcol.name)).set_text(cx, s);
    }
    /// Set line 2. An empty string hides the meta label (single-line card).
    pub fn set_meta(&mut self, cx: &mut Cx, s: &str) {
        self.view
            .widget(cx, ids!(textcol.meta))
            .set_visible(cx, !s.is_empty());
        self.view.label(cx, ids!(textcol.meta)).set_text(cx, s);
    }
    pub fn set_target(&mut self, key: &str, kind: ElementKind) {
        self.nav_key = key.to_string();
        self.nav_kind = Some(kind);
    }
    /// `Some((key, kind))` when this row emitted a click in `actions`.
    pub fn nav_target(&self, actions: &Actions) -> Option<(String, ElementKind)> {
        let clicked = actions
            .find_widget_action(self.widget_uid())
            .is_some_and(|a| matches!(a.cast(), RefCardViewAction::Clicked));
        if clicked {
            self.nav_kind.map(|k| (self.nav_key.clone(), k))
        } else {
            None
        }
    }
}

impl RefCardViewRef {
    pub fn set_icon(&self, cx: &mut Cx, icon: Icon) {
        if let Some(mut i) = self.borrow_mut() {
            i.set_icon(cx, icon);
        }
    }
    pub fn set_name(&self, cx: &mut Cx, s: &str) {
        if let Some(mut i) = self.borrow_mut() {
            i.set_name(cx, s);
        }
    }
    pub fn set_meta(&self, cx: &mut Cx, s: &str) {
        if let Some(mut i) = self.borrow_mut() {
            i.set_meta(cx, s);
        }
    }
    pub fn set_target(&self, key: &str, kind: ElementKind) {
        if let Some(mut i) = self.borrow_mut() {
            i.set_target(key, kind);
        }
    }
    pub fn nav_target(&self, actions: &Actions) -> Option<(String, ElementKind)> {
        self.borrow().and_then(|i| i.nav_target(actions))
    }
}
