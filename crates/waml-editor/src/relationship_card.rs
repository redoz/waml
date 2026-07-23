//! `RelationshipCardView`: one inspector relationship card -- a bordered rounded
//! rect (faint field-bg fill ringed by a low-alpha accent stroke, the working
//! box-radius idiom, never `sdf.box(..,0.0)` which floods this fork) holding a
//! Row(accent direction glyph + SemiBold name) over a dim meta line. Pure-view,
//! no interaction -- a `#[deref] View` hybrid mirroring `recent_row.rs`, values
//! pushed per row by the parent's FlatList loop.

use makepad_widgets::*;

script_mod! {
    use mod.prelude.widgets_internal.*
    use mod.atlas
    use mod.widgets.*
    use mod.text.*

    mod.widgets.RelationshipCardViewBase = #(RelationshipCardView::register_widget(vm))

    mod.widgets.RelationshipCardView = set_type_default() do mod.widgets.RelationshipCardViewBase{
        width: Fill
        height: Fit
        flow: Down
        padding: Inset{left: 10.0, right: 10.0, top: 10.0, bottom: 10.0}
        spacing: 2.0
        show_bg: true

        // Card material: faint field-bg fill + low-alpha accent ring, rounded
        // corners via the working box-radius idiom.
        draw_bg +: {
            color: atlas.field_bg
            border: uniform(atlas.accent)
            pixel: fn() {
                let sdf = Sdf2d.viewport(self.pos * self.rect_size)
                sdf.box(0.75, 0.75, self.rect_size.x - 1.5, self.rect_size.y - 1.5, 6.0)
                sdf.fill_keep(vec4(self.color.x, self.color.y, self.color.z, 0.5))
                sdf.stroke(vec4(self.border.x, self.border.y, self.border.z, 0.20), 1.0)
                return sdf.result
            }
        }

        headline := View {
            width: Fill
            height: Fit
            flow: Right
            align: Align{y: 0.5}
            spacing: 6.0

            glyph := Label {
                text: ""
                draw_text +: {
                    color: atlas.accent
                    text_style: TextStyle{
                        font_size: 13
                        font_family: FontFamily{
                            latin := FontMember{res: crate_resource("self:resources/fonts/IBM_Plex_Sans/IBMPlexSans-Regular.ttf") asc: -0.1 desc: 0.0}
                        }
                        line_spacing: 1.2
                    }
                }
            }
            name := Label {
                width: Fill
                text: ""
                draw_text +: {
                    color: atlas.text
                    text_style: TextStyle{
                        font_size: 12
                        font_family: FontFamily{
                            latin := FontMember{res: crate_resource("self:resources/fonts/IBM_Plex_Sans/IBMPlexSans-SemiBold.ttf") asc: -0.1 desc: 0.0}
                        }
                        line_spacing: 1.2
                    }
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
                    line_spacing: 1.2
                }
            }
        }
    }
}

#[derive(Script, ScriptHook, Widget)]
pub struct RelationshipCardView {
    #[deref]
    view: View,
}

impl Widget for RelationshipCardView {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);
    }
    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

impl RelationshipCardView {
    pub fn set_glyph(&mut self, cx: &mut Cx, s: &str) {
        self.view.label(cx, ids!(headline.glyph)).set_text(cx, s);
    }
    pub fn set_name(&mut self, cx: &mut Cx, s: &str) {
        self.view.label(cx, ids!(headline.name)).set_text(cx, s);
    }
    pub fn set_meta(&mut self, cx: &mut Cx, s: &str) {
        self.view.label(cx, ids!(meta)).set_text(cx, s);
    }
}

impl RelationshipCardViewRef {
    pub fn set_glyph(&self, cx: &mut Cx, s: &str) {
        if let Some(mut i) = self.borrow_mut() {
            i.set_glyph(cx, s);
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
}
