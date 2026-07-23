//! `AttrRowView`: one inspector attribute row, laid out `flow:Right` with real
//! alignment (NOT a concatenated string): optional visibility, an IBM Plex Mono
//! name, a literal ": ", an accent type, and a dim "[mult]". Pure-view, no
//! interaction -- a `#[deref] View` hybrid mirroring `recent_row.rs`, with
//! granular per-field setters the parent's FlatList loop pushes per row.

use makepad_widgets::*;

script_mod! {
    use mod.prelude.widgets_internal.*
    use mod.atlas
    use mod.widgets.*
    use mod.text.*

    mod.widgets.AttrRowViewBase = #(AttrRowView::register_widget(vm))

    mod.widgets.AttrRowView = set_type_default() do mod.widgets.AttrRowViewBase{
        width: Fill
        height: Fit
        flow: Right
        align: Align{y: 0.5}

        vis := Label {
            text: ""
            draw_text +: {
                color: atlas.text
                text_style: TextStyle{
                    font_size: 11
                    font_family: FontFamily{
                        latin := FontMember{res: crate_resource("self:resources/fonts/IBM_Plex_Mono/IBMPlexMono-Regular.ttf") asc: -0.1 desc: 0.0}
                    }
                    line_spacing: 1.2
                }
            }
        }
        name := Label {
            text: ""
            draw_text +: {
                color: atlas.text
                text_style: TextStyle{
                    font_size: 11
                    font_family: FontFamily{
                        latin := FontMember{res: crate_resource("self:resources/fonts/IBM_Plex_Mono/IBMPlexMono-Regular.ttf") asc: -0.1 desc: 0.0}
                    }
                    line_spacing: 1.2
                }
            }
        }
        colon := Label {
            text: ": "
            draw_text +: {
                color: atlas.text
                text_style: TextStyle{
                    font_size: 11
                    font_family: FontFamily{
                        latin := FontMember{res: crate_resource("self:resources/fonts/IBM_Plex_Mono/IBMPlexMono-Regular.ttf") asc: -0.1 desc: 0.0}
                    }
                    line_spacing: 1.2
                }
            }
        }
        ty := Label {
            text: ""
            draw_text +: {
                color: atlas.accent
                text_style: TextStyle{
                    font_size: 11
                    font_family: FontFamily{
                        latin := FontMember{res: crate_resource("self:resources/fonts/IBM_Plex_Mono/IBMPlexMono-Regular.ttf") asc: -0.1 desc: 0.0}
                    }
                    line_spacing: 1.2
                }
            }
        }
        mult := Label {
            text: ""
            draw_text +: {
                color: atlas.text_dim
                text_style: TextStyle{
                    font_size: 11
                    font_family: FontFamily{
                        latin := FontMember{res: crate_resource("self:resources/fonts/IBM_Plex_Mono/IBMPlexMono-Regular.ttf") asc: -0.1 desc: 0.0}
                    }
                    line_spacing: 1.2
                }
            }
        }
    }
}

#[derive(Script, ScriptHook, Widget)]
pub struct AttrRowView {
    #[deref]
    view: View,
}

impl Widget for AttrRowView {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);
    }
    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

impl AttrRowView {
    pub fn set_visibility(&mut self, cx: &mut Cx, s: &str) {
        self.view.label(cx, ids!(vis)).set_text(cx, s);
    }
    pub fn set_name(&mut self, cx: &mut Cx, s: &str) {
        self.view.label(cx, ids!(name)).set_text(cx, s);
    }
    pub fn set_ty(&mut self, cx: &mut Cx, s: &str) {
        self.view.label(cx, ids!(ty)).set_text(cx, s);
    }
    pub fn set_mult(&mut self, cx: &mut Cx, s: &str) {
        self.view.label(cx, ids!(mult)).set_text(cx, s);
    }
}

impl AttrRowViewRef {
    pub fn set_visibility(&self, cx: &mut Cx, s: &str) {
        if let Some(mut i) = self.borrow_mut() {
            i.set_visibility(cx, s);
        }
    }
    pub fn set_name(&self, cx: &mut Cx, s: &str) {
        if let Some(mut i) = self.borrow_mut() {
            i.set_name(cx, s);
        }
    }
    pub fn set_ty(&self, cx: &mut Cx, s: &str) {
        if let Some(mut i) = self.borrow_mut() {
            i.set_ty(cx, s);
        }
    }
    pub fn set_mult(&self, cx: &mut Cx, s: &str) {
        if let Some(mut i) = self.borrow_mut() {
            i.set_mult(cx, s);
        }
    }
}
