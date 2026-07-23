//! `SectionHeading`: one Atlas "eyebrow" label (small SemiBold, `text_dim`) for
//! the inspector body's ATTRIBUTES / RELATIONSHIPS / DESCRIPTION dividers.
//! Pure-view, no interaction: a `#[deref] View` hybrid mirroring `recent_row.rs`
//! with a single `set_text` setter the parent pushes per draw. Reusable for node
//! cards and the node editor.

use makepad_widgets::*;

script_mod! {
    use mod.prelude.widgets_internal.*
    use mod.atlas
    use mod.widgets.*
    use mod.text.*

    mod.widgets.SectionHeadingBase = #(SectionHeading::register_widget(vm))

    mod.widgets.SectionHeading = set_type_default() do mod.widgets.SectionHeadingBase{
        width: Fill
        height: Fit

        label := Label {
            text: ""
            draw_text +: {
                color: atlas.text_dim
                text_style: TextStyle{
                    font_size: 10
                    font_family: FontFamily{
                        latin := FontMember{res: crate_resource("self:resources/fonts/IBM_Plex_Sans/IBMPlexSans-SemiBold.ttf") asc: -0.1 desc: 0.0}
                    }
                    line_spacing: 1.2
                }
            }
        }
    }
}

#[derive(Script, ScriptHook, Widget)]
pub struct SectionHeading {
    #[deref]
    view: View,
}

impl Widget for SectionHeading {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);
    }
    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

impl SectionHeading {
    /// Set the eyebrow text (e.g. "ATTRIBUTES").
    pub fn set_text(&mut self, cx: &mut Cx, s: &str) {
        self.view.label(cx, ids!(label)).set_text(cx, s);
    }
}

impl SectionHeadingRef {
    pub fn set_text(&self, cx: &mut Cx, s: &str) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.set_text(cx, s);
        }
    }
}
