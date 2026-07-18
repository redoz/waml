//! The `Inspector` widget: a right-side panel that renders an `InspectorView`
//! (see `inspector.rs`) as typeset text. Drawn immediate-mode with `DrawText`,
//! exactly like `GraphCanvas` draws node titles — no dynamic child widgets. This
//! is the read-only first cut (scope A, step A); inline editing + in-memory
//! overrides are a fast-follow.

use crate::inspector::{build_view, InspectorView, Subject};
use makepad_widgets::*;
use waml::model::Model;

script_mod! {
    use mod.prelude.widgets_internal.*
    use mod.widgets.*
    use mod.text.*

    mod.widgets.InspectorBase = #(Inspector::register_widget(vm))

    mod.widgets.Inspector = set_type_default() do mod.widgets.InspectorBase{
        width: Fill
        height: Fill
        draw_bg +: { color: #x1b1b24 }
        draw_title +: {
            color: #xf0f0f6
            text_style: TextStyle{
                font_size: 16
                font_family: FontFamily{
                    latin := FontMember{res: crate_resource("self:resources/fonts/IBMPlexSans-Regular.ttf") asc: -0.1 desc: 0.0}
                }
                line_spacing: 1.2
            }
        }
        draw_label +: {
            color: #xc8c8d4
            text_style: TextStyle{
                font_size: 12
                font_family: FontFamily{
                    latin := FontMember{res: crate_resource("self:resources/fonts/IBMPlexSans-Regular.ttf") asc: -0.1 desc: 0.0}
                }
                line_spacing: 1.2
            }
        }
        draw_dim +: {
            color: #x9a9aae
            text_style: TextStyle{
                font_size: 12
                font_family: FontFamily{
                    latin := FontMember{res: crate_resource("self:resources/fonts/IBMPlexSans-Regular.ttf") asc: -0.1 desc: 0.0}
                }
                line_spacing: 1.2
            }
        }
    }
}

#[derive(Script, ScriptHook, Widget)]
pub struct Inspector {
    #[uid]
    uid: WidgetUid,
    #[source]
    source: ScriptObjectRef,
    #[walk]
    walk: Walk,
    #[layout]
    layout: Layout,

    #[redraw]
    #[live]
    draw_bg: DrawColor,
    #[redraw]
    #[live]
    draw_title: DrawText,
    #[redraw]
    #[live]
    draw_label: DrawText,
    #[redraw]
    #[live]
    draw_dim: DrawText,

    #[rust]
    view: Option<InspectorView>,
    #[rust]
    view_rect: Rect,
}

// Panel geometry (px). Fixed line advances — no text measuring in this cut.
const PAD: f64 = 16.0;
const TITLE_H: f64 = 26.0;
const ROW_H: f64 = 20.0;
const GAP: f64 = 12.0;

impl Widget for Inspector {
    fn handle_event(&mut self, _cx: &mut Cx, _event: &Event, _scope: &mut Scope) {
        // Read-only in this cut: nothing to handle.
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, _scope: &mut Scope, walk: Walk) -> DrawStep {
        let rect = cx.walk_turtle(walk);
        self.view_rect = rect;
        self.draw_bg.draw_abs(cx, rect);

        let Some(view) = self.view.clone() else {
            // Empty state: one quiet centered line.
            let pos = dvec2(rect.pos.x + PAD, rect.pos.y + rect.size.y * 0.5 - ROW_H);
            self.draw_dim.draw_abs(cx, pos, "Select an element");
            return DrawStep::done();
        };

        let x = rect.pos.x + PAD;
        let mut y = rect.pos.y + PAD;

        self.draw_title.draw_abs(cx, dvec2(x, y), &view.title);
        y += TITLE_H;
        self.draw_dim.draw_abs(cx, dvec2(x, y), &view.kind_label);
        y += ROW_H + GAP;

        if !view.attributes.is_empty() {
            self.draw_dim.draw_abs(cx, dvec2(x, y), "ATTRIBUTES");
            y += ROW_H;
            for attr in &view.attributes {
                let vis = if attr.visibility.is_empty() {
                    String::new()
                } else {
                    format!("{} ", attr.visibility)
                };
                let mult = if attr.multiplicity.is_empty() || attr.multiplicity == "1" {
                    String::new()
                } else {
                    format!("  [{}]", attr.multiplicity)
                };
                let line = format!("{vis}{}: {}{mult}", attr.name, attr.ty);
                self.draw_label.draw_abs(cx, dvec2(x, y), &line);
                y += ROW_H;
            }
            y += GAP;
        }

        if let Some(desc) = &view.description {
            self.draw_dim.draw_abs(cx, dvec2(x, y), "DESCRIPTION");
            y += ROW_H;
            self.draw_label.draw_abs(cx, dvec2(x, y), desc);
        }

        DrawStep::done()
    }
}

impl Inspector {
    /// Point the inspector at `subject`, rebuilding the projection and redrawing.
    pub fn set_subject(&mut self, cx: &mut Cx, model: &Model, subject: Subject) {
        self.view = build_view(model, &subject);
        self.draw_bg.redraw(cx);
    }
}
