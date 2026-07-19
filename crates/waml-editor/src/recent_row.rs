//! `RecentRowView`: one recent-project row on the start screen. Renders
//! `[accent marker] [title / path stacked] .......... [timestamp flush-right]`
//! purely from the makepad layout engine -- no text measurement, no y-offsets.
//! The timestamp is right-anchored by the `Fill` width on the middle text
//! column, which consumes all slack and shoves the `Fit`-width `when` label to
//! the right edge.
//!
//! Task 3 of the start-screen recents refactor adds interaction: the row now
//! hit-tests its own area, emits a `RecentRowViewAction::Clicked` widget-action
//! on `FingerUp` (read by `StartScreen` through `FlatList::items_with_actions`),
//! and self-manages a subtle hover wash driven by FingerHoverIn/Out -- the
//! `#[deref] View` hybrid pattern (same as `inspector_panel.rs`), with granular
//! per-line setters the parent calls per row.

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
        padding: Inset{left: 12.0, right: 12.0, top: 5.0, bottom: 5.0}
        spacing: 12.0
        show_bg: true

        // Hover wash: a subtle premultiplied accent fill behind the whole row,
        // faded by the `hover` uniform (0 rest / 1 pointer-over) the widget sets
        // from FingerHoverIn/Out. Full-rect return (no sdf.box, which floods at
        // 0-radius in this fork), premultiplied like `CardShadow` so a low-alpha
        // tint reads as a wash, not a bloom.
        draw_bg +: {
            color: atlas.accent
            hover: uniform(0.0)
            pixel: fn() {
                let a = 0.12 * self.hover
                return vec4(self.color.x * a, self.color.y * a, self.color.z * a, a)
            }
        }

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

        // Title line over path, stacked. The timestamp rides the TITLE line
        // (shares its horizontal centerline with `title`), so the path spans
        // the full column width below. `title` is `Fill` inside `titlerow`,
        // eating the slack and shoving the `Fit` `when` flush right -- the
        // whole right-anchor mechanism, no measuring.
        textcol := View {
            width: Fill
            height: Fit
            flow: Down
            spacing: 0.0

            titlerow := View {
                width: Fill
                height: Fit
                flow: Right
                align: Align{y: 0.5}
                title := Label {
                    width: Fill
                    text: ""
                    draw_text +: {
                        color: atlas.text
                        text_style: theme.font_regular{font_size: 12 line_spacing: 1.0}
                    }
                }
                // Right-anchored last-opened stamp. `Fit` width -> `title`'s
                // `Fill` shoves it to the right edge, on the title's line.
                when := Label {
                    text: ""
                    draw_text +: {
                        color: atlas.text_dim
                        text_style: theme.font_regular{font_size: 10 line_spacing: 1.0}
                    }
                }
            }

            path := Label {
                text: ""
                draw_text +: {
                    color: atlas.text_dim
                    text_style: theme.font_regular{font_size: 10 line_spacing: 1.0}
                }
            }
        }
    }
}

/// Emitted (grouped through the parent `FlatList`) when a row is clicked.
/// `StartScreen::handle_actions` reads it via `items_with_actions` +
/// `RecentRowViewRef::clicked` and maps the row back to a recent index.
#[derive(Clone, Debug, Default)]
pub enum RecentRowViewAction {
    #[default]
    None,
    Clicked,
}

#[derive(Script, ScriptHook, Widget)]
pub struct RecentRowView {
    /// The row container: the marker + stacked text column + timestamp declared
    /// in the DSL tree above.
    #[deref]
    view: View,

    /// Pointer-over state, self-managed from FingerHoverIn/Out; fed to the
    /// `hover` uniform on the root `draw_bg` each `draw_walk` for the wash.
    #[rust]
    hovered: bool,
    /// Whether this row responds to hover/click. Real recents set it true; the
    /// empty-state placeholder row leaves it false so it neither washes nor fires.
    #[rust]
    clickable: bool,
}

impl Widget for RecentRowView {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, _scope: &mut Scope) {
        if !self.clickable {
            return;
        }
        let uid = self.widget_uid();
        match event.hits(cx, self.view.area()) {
            Hit::FingerUp(fe) if fe.is_primary_hit() && fe.is_over => {
                cx.widget_action(uid, RecentRowViewAction::Clicked);
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
    // container's child Labels render.
    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view
            .draw_bg
            .set_uniform(cx, live_id!(hover), &[if self.hovered { 1.0 } else { 0.0 }]);
        self.view.draw_walk(cx, scope, walk)
    }
}

impl RecentRowView {
    /// Set the bold project title (top line).
    pub fn set_title(&mut self, cx: &mut Cx, s: &str) {
        self.view.label(cx, ids!(textcol.titlerow.title)).set_text(cx, s);
    }
    /// Set the dim project path (second line).
    pub fn set_path(&mut self, cx: &mut Cx, s: &str) {
        self.view.label(cx, ids!(textcol.path)).set_text(cx, s);
    }
    /// Set the right-anchored last-opened stamp.
    pub fn set_when(&mut self, cx: &mut Cx, s: &str) {
        self.view.label(cx, ids!(textcol.titlerow.when)).set_text(cx, s);
    }
    /// Toggle whether the row hovers/clicks (false for the empty-state row).
    pub fn set_clickable(&mut self, clickable: bool) {
        self.clickable = clickable;
    }
    /// True when this row emitted a click in `actions`.
    pub fn clicked(&self, actions: &Actions) -> bool {
        actions
            .find_widget_action(self.widget_uid())
            .is_some_and(|a| matches!(a.cast(), RecentRowViewAction::Clicked))
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
    pub fn set_clickable(&self, clickable: bool) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.set_clickable(clickable);
        }
    }
    /// See [`RecentRowView::clicked`].
    pub fn clicked(&self, actions: &Actions) -> bool {
        self.borrow().is_some_and(|inner| inner.clicked(actions))
    }
}
