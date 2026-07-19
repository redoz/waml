//! `RecentRowView`: one recent-project row on the start screen. Renders
//! `[accent marker] [title / path stacked] .......... [timestamp flush-right]`
//! purely from the makepad layout engine -- no text measurement, no y-offsets.
//! The timestamp is right-anchored by the `Fill` width on the middle text
//! column, which consumes all slack and shoves the `Fit`-width `when` label to
//! the right edge.
//!
//! Task 2 of the start-screen recents refactor: this replaces the title-only
//! placeholder `Row` template `start_screen.rs` drove in Task 1. Presentation
//! only -- the `#[deref] View` hybrid pattern (same as `inspector_panel.rs`),
//! with granular per-line setters the parent calls per row. Click routing +
//! hover land in Task 3, so there is no `handle_event`/action surface here.

use makepad_widgets::*;

script_mod! {
    use mod.prelude.widgets_internal.*
    use mod.atlas
    use mod.widgets.*
    use mod.text.*

    mod.widgets.RecentRowViewBase = #(RecentRowView::register_widget(vm))

    mod.widgets.RecentRowView = set_type_default() do mod.widgets.RecentRowViewBase{
        width: Fill
        height: Fit
        flow: Right
        align: Align{y: 0.5}
        padding: Inset{left: 12.0, right: 12.0, top: 8.0, bottom: 8.0}
        spacing: 12.0

        // Accent bullet: a sharp 7x7 solid square (0-radius `sdf.box` floods
        // this fork, so fill an `sdf.rect`, matching the `rule`/button
        // placeholders in `start_screen.rs`). Vertically centered by the row's
        // `align`.
        marker := View {
            width: 7.0
            height: 7.0
            show_bg: true
            draw_bg +: {
                color: atlas.accent
                pixel: fn() {
                    let sdf = Sdf2d.viewport(self.pos * self.rect_size)
                    sdf.rect(0.0, 0.0, self.rect_size.x, self.rect_size.y)
                    sdf.fill(self.color)
                    return sdf.result
                }
            }
        }

        // Title over path, stacked. `Fill` width so this column eats all the
        // slack and pushes `when` flush to the right edge -- the whole
        // right-anchor mechanism, no measuring.
        textcol := View {
            width: Fill
            height: Fit
            flow: Down
            spacing: 2.0
            title := Label {
                text: ""
                draw_text +: {
                    color: atlas.text
                    text_style: theme.font_regular{font_size: 12 line_spacing: 1.2}
                }
            }
            path := Label {
                text: ""
                draw_text +: {
                    color: atlas.text_dim
                    text_style: theme.font_regular{font_size: 10 line_spacing: 1.2}
                }
            }
        }

        // Right-anchored last-opened stamp. `Fit` width -> the `Fill` on
        // `textcol` shoves it to the right edge.
        when := Label {
            text: ""
            draw_text +: {
                color: atlas.text_dim
                text_style: theme.font_regular{font_size: 10 line_spacing: 1.2}
            }
        }
    }
}

#[derive(Script, ScriptHook, Widget)]
pub struct RecentRowView {
    /// The row container: the marker + stacked text column + timestamp declared
    /// in the DSL tree above.
    #[deref]
    view: View,
}

impl Widget for RecentRowView {
    // Presentation only: delegate the draw to the container so its child Labels
    // render. No `handle_event` (the Widget default no-op suffices) -- click
    // routing / hover are Task 3.
    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

impl RecentRowView {
    /// Set the bold project title (top line).
    pub fn set_title(&mut self, cx: &mut Cx, s: &str) {
        self.view.label(cx, ids!(textcol.title)).set_text(cx, s);
    }
    /// Set the dim project path (second line).
    pub fn set_path(&mut self, cx: &mut Cx, s: &str) {
        self.view.label(cx, ids!(textcol.path)).set_text(cx, s);
    }
    /// Set the right-anchored last-opened stamp.
    pub fn set_when(&mut self, cx: &mut Cx, s: &str) {
        self.view.label(cx, ids!(when)).set_text(cx, s);
    }
}

impl RecentRowViewRef {
    /// `WidgetRef`-side setters, so the FlatList draw loop can push per-row text
    /// through `row.as_recent_row_view()` without borrowing the inner widget by
    /// hand.
    pub fn set_title(&self, cx: &mut Cx, s: &str) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.set_title(cx, s);
        }
    }
    pub fn set_path(&self, cx: &mut Cx, s: &str) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.set_path(cx, s);
        }
    }
    pub fn set_when(&self, cx: &mut Cx, s: &str) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.set_when(cx, s);
        }
    }
}
