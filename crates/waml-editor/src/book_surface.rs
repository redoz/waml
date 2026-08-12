//! The book's one drawing surface: a virtualized vertical scroll of
//! sections. Owns its scroll offset (like `tree_panel`'s `TreeLayout`)
//! because a `View`-owned scroll cannot drive the live-child window;
//! sections draw at fixed or Fit heights, NEVER Fill -- a `draw_walk` rect
//! goes stale after a `Size::Fill` sibling, which would corrupt the
//! measured-height cache.
//!
//! Only Prose/Diagram sections hold a live child widget: a `MarkdownViewer`
//! fed the compiled `ReadingDocument`, or a `ClassDiagramSurface` given the
//! built `Scene` with interaction off. Heading and Link sections draw
//! immediate-mode, the same hand-drawn style `tree_row_draw.rs` uses for
//! tree rows, and are positioned from this widget's own `heights`/`tops` --
//! not from the turtle -- so the offset math stays in one place
//! (`book_layout.rs`). A diagram's "open full" affordance and every Link
//! row are ALSO hand-drawn (no real button/row widget), so their hit rects
//! are cached every draw pass (the `folder_list.rs` `row_rects` pattern)
//! and hit-tested directly in `handle_event`.

use std::collections::HashMap;
use std::rc::Rc;

use makepad_widgets::*;
use waml_markdown_editor::reading::{MarkdownViewer, MarkdownViewerWidgetRefExt};

use crate::book_model::{BookModel, BookSection, LinkReason, SectionBody};
use crate::icons::{Icon, IconSet};
use waml::view::row::RowId;

/// A hand-drawn clickable region the pointer can be over: a Link row or a
/// diagram's open-full affordance. Distinct from [`BookSurfaceAction`] (the
/// action a CLICK emits) because hover needs to compare "is this the same
/// target as last frame", not just "did a click land".
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BookHoverTarget {
    Link(usize),
    OpenFull(usize),
}

/// Emitted when the reader clicks a hand-drawn Link row or a diagram's
/// open-full affordance -- both index `BookModel::sections` (the same
/// indices `set_model` was last given). `BookView::handle` maps either
/// through `navigation_for_section`. `FoldMoved` carries the section the
/// fold just crossed into -- emitted per CROSSING, never per scroll tick
/// (the `last_marked` gate) -- and becomes a tree mark, not a navigation.
#[derive(Clone, Debug, Default)]
pub enum BookSurfaceAction {
    #[default]
    None,
    LinkClicked(usize),
    OpenFullClicked(usize),
    FoldMoved(usize),
}

script_mod! {
    use mod.prelude.widgets_internal.*
    use mod.atlas
    use mod.widgets.*
    use mod.text.*
    use mod.fonts

    mod.widgets.BookSurfaceBase = #(BookSurface::register_widget(vm))
    mod.widgets.BookSurface = set_type_default() do mod.widgets.BookSurfaceBase{
        width: Fill
        height: Fill

        draw_heading +: {
            color: atlas.text
            text_style: fonts.text_heading
        }
        draw_link +: {
            color: atlas.text_dim
            text_style: fonts.text_label
        }
        // Colour-only holder (never drawn): the immediate-mode link glyph
        // copies `color` from this per draw, the `FolderRow` pattern -- no
        // RGBA crosses Rust. Also tints the diagram caption's open-full
        // glyph -- both are muted affordance icons.
        draw_link_icon +: { color: atlas.text_dim }
        // A diagram section's caption-strip title, above its live embed.
        draw_caption +: {
            color: atlas.text
            text_style: fonts.text_label
        }
        // Hover wash for a Link row or the diagram open-full affordance,
        // painted BENEATH the row's own glyph/text -- the `tree_panel.rs`
        // `draw_hover` pattern (flat, full-bleed, no corner radius).
        draw_row_hover +: { color: atlas.hover }
        // The hairline under a heading row.
        draw_rule +: { color: atlas.text_dim }
        // The scroll bar, drawn from this widget's own offset (the
        // `tree_panel.rs` pattern -- rows here are positioned the same way).
        draw_scrollbar: mod.draw.DrawColor{
            color: atlas.text_dim
            pixel: fn() {
                let sdf = Sdf2d.viewport(self.pos * self.rect_size)
                sdf.box(0.5, 0.5, self.rect_size.x - 1.0, self.rect_size.y - 1.0, 2.0)
                sdf.fill(self.color)
                return sdf.result
            }
        }
    }
}

/// Left indent per depth level. Reuses `tree_layout`'s per-depth step so a
/// book section's indent reads the same as the tree row it mirrors.
const DEPTH_INDENT: f64 = crate::tree_layout::ICON_DEPTH_INDENT;
const HEADING_LEFT: f64 = 8.0;
const HEADING_TOP_PAD: f64 = 14.0;
const LINK_ICON_SIZE: f64 = 14.0;
const LINK_LEFT: f64 = 8.0;
/// Side of the diagram caption's open-full glyph square.
const OPEN_FULL_ICON_SIZE: f64 = 16.0;

#[derive(Script, ScriptHook, Widget)]
pub struct BookSurface {
    #[deref]
    view: View,
    /// SDF icon set for a link row's leading glyph, drawn via `IconSet::draw`
    /// (the `folder_list.rs`/`tree_panel.rs` pattern).
    #[live]
    icons: IconSet,
    #[live]
    draw_heading: DrawText,
    #[live]
    draw_link: DrawText,
    #[live]
    draw_link_icon: DrawColor,
    /// A diagram section's caption-strip title.
    #[live]
    draw_caption: DrawText,
    /// Hover wash under a Link row or the diagram open-full affordance.
    #[live]
    draw_row_hover: DrawColor,
    #[live]
    draw_rule: DrawColor,
    #[live]
    draw_scrollbar: DrawColor,
    #[rust]
    model: Option<Rc<BookModel>>,
    /// Measured heights by `RowId` -- survives a model rebuild so a reloaded
    /// book keeps its layout; a section whose body changed is re-estimated
    /// the next time it goes live (the cache is never explicitly
    /// invalidated, only overwritten by the next real measurement).
    #[rust]
    measured: HashMap<RowId, f64>,
    #[rust]
    heights: Vec<f64>,
    #[rust]
    tops: Vec<f64>,
    #[rust]
    scroll: f64,
    /// Live child widgets, keyed by section index -- the window
    /// `book_layout::live_window` names. Cleared and lazily repopulated on
    /// every model swap (`set_model`), since indices from the old model may
    /// no longer name the same section.
    #[rust]
    live: HashMap<usize, WidgetRef>,
    #[rust]
    last_viewport: f64,
    /// This widget's absolute rect from the last `draw_walk`, cached so
    /// `handle_event` (scroll wheel, scrollbar drag) can hit-test and clamp
    /// without waiting for another draw -- the same reason `TreeLayout`
    /// caches its own viewport.
    #[rust]
    viewport_rect: Rect,
    /// Pointer y-offset from the scrollbar thumb's top while dragging;
    /// `None` when no drag is in flight. Mirrors `tree_panel`'s
    /// `scroll_grab`.
    #[rust]
    scroll_grab: Option<f64>,
    /// The last section index `FoldMoved` was emitted for -- the change gate
    /// that keeps the mark per-crossing rather than per scroll tick. `None`
    /// until the reader's first scroll, so opening a book does not pulse the
    /// tree unprompted.
    #[rust]
    last_marked: Option<usize>,
    /// Absolute rects of the currently-drawn Link rows, indexed by section
    /// index, rebuilt every draw pass -- `folder_list.rs`'s `row_rects`
    /// pattern, since these rows are hand-drawn, not real child widgets.
    #[rust]
    link_rects: Vec<(usize, Rect)>,
    /// Absolute rects of the currently-drawn diagram open-full affordance,
    /// same shape as `link_rects`.
    #[rust]
    open_full_rects: Vec<(usize, Rect)>,
    /// The clickable target under the pointer, if any -- drives the hover
    /// cursor and the row/button hover wash. `None` off any clickable rect.
    #[rust]
    hovered: Option<BookHoverTarget>,
    /// The hand-drawn target the current primary press started on, if any --
    /// a release only clicks when it lands on the same target. `None` when
    /// no press is in flight (or the press started off any clickable rect).
    #[rust]
    pressed: Option<BookHoverTarget>,
}

impl Widget for BookSurface {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);

        // Hand-drawn scroll bar, the exact `tree_panel.rs` drag shape: a
        // primary press on the thumb starts a drag tracked by the pointer's
        // offset from the thumb top, a move maps the pointer back to a
        // scroll offset, and the release ends it. Each returns early so the
        // same press is not also read as a wheel/row event below. The press
        // respects `e.handled` so a popup that already claimed it (popups
        // stamp MouseDown/MouseMove) cannot also start a thumb drag.
        match event {
            Event::MouseDown(e) if e.button.is_primary() && e.handled.get().is_empty() => {
                if let Some(thumb) = self.thumb_rect() {
                    if thumb.contains(e.abs) {
                        self.scroll_grab = Some(e.abs.y - thumb.pos.y);
                        e.handled.set(self.view.area());
                        return;
                    }
                }
            }
            Event::MouseMove(e) => {
                if let Some(grab) = self.scroll_grab {
                    let before = self.scroll;
                    self.set_scroll(self.scroll_for_thumb_y(e.abs.y - grab));
                    if self.scroll != before {
                        self.emit_fold_crossing(cx);
                        self.reconcile_live(cx, self.last_viewport);
                        self.view.redraw(cx);
                    }
                    return;
                }
            }
            Event::MouseUp(e) if e.button.is_primary() && self.scroll_grab.is_some() => {
                self.scroll_grab = None;
                return;
            }
            _ => {}
        }

        // Everything else -- clicks on the hand-drawn Link rows and the
        // diagram open-full affordance, hover, and wheel scroll -- routes
        // through the hits system, the same machinery real tree/folder rows
        // sit behind. Unlike the raw Mouse events above, hits respect
        // `e.handled` (a press a popup already claimed never reaches here),
        // clip every test to the view's own rect (a cached row rect can
        // extend past the viewport, since the draw loop culls only fully
        // invisible sections), and deliver a FingerUp only to the widget
        // whose FingerDown captured the pointer -- so a click that started
        // on a popup or another widget can never release into a navigation
        // here. Scroll keeps its capture_overload, as before, so a fling
        // over child content still reaches this owner of the offset.
        let hit = if matches!(event, Event::Scroll(_)) {
            event.hits_with_capture_overload(cx, self.view.area(), true)
        } else {
            event.hits(cx, self.view.area())
        };
        match hit {
            Hit::FingerDown(fe) if fe.is_primary_hit() => {
                // Arm the press: a release only clicks when it lands on the
                // SAME hand-drawn target it started on -- the real-button
                // contract, and (with `set_model` clearing `pressed`) the
                // guard against a release hit-testing rects cached for a
                // model that was swapped out mid-click.
                self.pressed = self.hover_target_at(fe.abs);
            }
            Hit::FingerUp(fe) if fe.is_primary_hit() => {
                let pressed = self.pressed.take();
                if fe.is_over {
                    if let Some(target) = self.hover_target_at(fe.abs) {
                        if pressed == Some(target) {
                            let uid = self.widget_uid();
                            match target {
                                BookHoverTarget::Link(index) => {
                                    cx.widget_action(uid, BookSurfaceAction::LinkClicked(index));
                                }
                                BookHoverTarget::OpenFull(index) => {
                                    cx.widget_action(
                                        uid,
                                        BookSurfaceAction::OpenFullClicked(index),
                                    );
                                }
                            }
                        }
                    }
                }
            }
            // Hover cursor + wash for the hand-drawn rows -- none is a real
            // child widget, so this widget hit-tests its own cached rects.
            Hit::FingerHoverIn(fe) | Hit::FingerHoverOver(fe) => {
                let target = self.hover_target_at(fe.abs);
                self.update_hover(cx, target);
            }
            Hit::FingerHoverOut(_) => {
                self.update_hover(cx, None);
            }
            // Wheel/trackpad scroll. This widget owns the offset and clamps
            // it, so a fling past either end simply stops rather than
            // stranding the sections.
            Hit::FingerScroll(fe) => {
                let before = self.scroll;
                self.set_scroll(before + fe.scroll.y);
                if self.scroll != before {
                    self.emit_fold_crossing(cx);
                    self.reconcile_live(cx, self.last_viewport);
                    self.view.redraw(cx);
                }
            }
            _ => {}
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        while self.view.draw_walk(cx, scope, walk).step().is_some() {}
        let rect = self.view.area().rect(cx);
        self.viewport_rect = rect;
        self.reconcile_live(cx, rect.size.y);

        cx.push_clip_rect(rect);

        // Rebuilt every pass: only currently-visible rows/buttons are
        // clickable, and the loop below repopulates exactly those that draw
        // this frame (`folder_list.rs`'s `row_rects` reset).
        self.link_rects.clear();
        self.open_full_rects.clear();

        let mut remeasured = false;
        if let Some(model) = self.model.clone() {
            for (index, section) in model.sections.iter().enumerate() {
                let top = self.tops.get(index).copied().unwrap_or(0.0);
                let height = self.heights.get(index).copied().unwrap_or(0.0);
                let y = rect.pos.y + top - self.scroll;
                if y + height < rect.pos.y || y > rect.pos.y + rect.size.y {
                    continue; // fully outside the clipped body: nothing to paint
                }
                let section_rect = Rect {
                    pos: dvec2(rect.pos.x, y),
                    size: dvec2(rect.size.x, height),
                };
                match &section.body {
                    SectionBody::Heading => self.draw_heading_row(cx, section, section_rect),
                    SectionBody::Link { reason } => {
                        self.draw_link_row(cx, index, section, reason, section_rect)
                    }
                    SectionBody::Prose { .. } => {
                        if let Some(child) = self.live.get(&index) {
                            // Fixed width, FIT height: the child must be free
                            // to draw its natural height, or there is nothing
                            // to measure -- `Walk::abs_rect` would pin height
                            // to the cached value, and the measurement below
                            // could only echo the estimate back forever.
                            let walk = Walk {
                                abs_pos: Some(section_rect.pos),
                                margin: Inset::default(),
                                width: Size::Fixed(section_rect.size.x),
                                height: Size::fit(),
                                metrics: Metrics::default(),
                            };
                            let _ = child.draw_walk(cx, scope, walk);
                            // Measured via the viewer's own flow-height probe:
                            // with a document installed, `MarkdownViewer` drives
                            // a `TextFlow` and never draws its `view`, so the
                            // child's widget area is stale/empty and cannot be
                            // read for the drawn height.
                            let drawn = child.as_markdown_viewer().content_height();
                            if self.record_measured(section, drawn) {
                                remeasured = true;
                            }
                        }
                    }
                    SectionBody::Diagram { .. } => {
                        // Fixed height by design (the embed cap, not the
                        // child's natural size): the caption strip plus
                        // whatever remains of `section_rect` for the canvas,
                        // never re-measured.
                        let caption_h = crate::book_layout::DIAGRAM_CAPTION_HEIGHT.min(height);
                        let caption_rect = Rect {
                            pos: section_rect.pos,
                            size: dvec2(section_rect.size.x, caption_h),
                        };
                        self.draw_diagram_caption(cx, index, section, caption_rect);
                        let canvas_rect = Rect {
                            pos: dvec2(section_rect.pos.x, section_rect.pos.y + caption_h),
                            size: dvec2(section_rect.size.x, (height - caption_h).max(0.0)),
                        };
                        if let Some(child) = self.live.get(&index) {
                            let _ = child.draw_walk(cx, scope, Walk::abs_rect(canvas_rect));
                        }
                    }
                }
            }
        }

        if let Some(thumb) = self.thumb_rect() {
            self.draw_scrollbar.draw_abs(cx, thumb);
        }

        cx.pop_clip_rect();

        if remeasured {
            // A live section drew at a different height than its estimate
            // (or its last measurement) -- rebuild `tops` so the scrollbar
            // and every section below it stay in agreement with what was
            // actually drawn. `set_model` already re-clamps `scroll` on a
            // full rebuild; here the offset itself did not move. Redrawing
            // lets the next pass paint every later section at its corrected
            // position instead of leaving them stale until some unrelated
            // event asks for a redraw.
            self.rebuild_layout();
            self.view.redraw(cx);
        }

        DrawStep::done()
    }
}

impl BookSurface {
    pub fn set_model(&mut self, cx: &mut Cx, model: Rc<BookModel>) {
        self.live.clear(); // children are per-section; a swap re-creates lazily
                           // The cached hit rects -- and any hover/press armed on them -- index
                           // the OLD model's sections. A click or hover landing between this
                           // swap and the next draw must not hit-test them: `.get` would keep it
                           // from panicking, but it would silently act on the wrong section.
        self.link_rects.clear();
        self.open_full_rects.clear();
        self.pressed = None;
        if self.hovered.take().is_some() {
            crate::cursor::hover_out(cx);
        }
        // A rebuild of the SAME book keeps the reader's place (the clamp
        // below); a DIFFERENT book must not inherit it -- this widget is the
        // one shared body every book tab draws on, so without this reset a
        // nested book opened from deep in `/guide` would start at `/guide`'s
        // offset, and `last_marked` would index the previous model's
        // sections. `measured` deliberately survives either way: it is keyed
        // by `RowId`, which is collision-free across books.
        if self
            .model
            .as_ref()
            .is_some_and(|current| current.directory != model.directory)
        {
            self.scroll = 0.0;
            self.last_marked = None;
        }
        self.model = Some(model);
        self.rebuild_layout();
        let total: f64 = self.heights.iter().sum();
        self.scroll = self.scroll.min((total - self.last_viewport).max(0.0));
        self.view.redraw(cx);
    }

    /// Record one prose section's freshly-drawn height into the
    /// `RowId`-keyed cache; true when the cache changed and the caller must
    /// re-layout. The `> 0.0` guard skips the sentinel `content_height`
    /// reports before the child's flow has drawn a document.
    fn record_measured(&mut self, section: &BookSection, drawn: f64) -> bool {
        if drawn > 0.0 && self.measured.get(&section.row_id).copied() != Some(drawn) {
            self.measured.insert(section.row_id.clone(), drawn);
            return true;
        }
        false
    }

    fn rebuild_layout(&mut self) {
        let Some(model) = self.model.clone() else {
            self.heights.clear();
            self.tops.clear();
            return;
        };
        self.heights = model
            .sections
            .iter()
            .map(|s| {
                self.measured
                    .get(&s.row_id)
                    .copied()
                    .unwrap_or_else(|| crate::book_layout::estimated_height(&s.body))
            })
            .collect();
        self.tops = crate::book_layout::section_tops(&self.heights);
    }

    pub(crate) fn reconcile_live(&mut self, cx: &mut Cx, viewport_height: f64) {
        self.last_viewport = viewport_height;
        let window = crate::book_layout::live_window(
            &self.tops,
            &self.heights,
            self.scroll,
            viewport_height,
        );
        self.live.retain(|index, _| window.contains(index));
        let Some(model) = self.model.clone() else {
            return;
        };
        for index in window {
            if self.live.contains_key(&index) {
                continue;
            }
            if let Some(child) = self.make_child(cx, &model.sections[index]) {
                self.live.insert(index, child);
            }
        }
    }

    /// The live child for a Prose/Diagram section: a `MarkdownViewer` fed
    /// the compiled `ReadingDocument` (the exact `install_document` call
    /// `reading_view.rs` makes), or a `ClassDiagramSurface` given the built
    /// `Scene` with interaction off -- the surface fits the scene to
    /// whatever rect it is drawn at, so the fixed embed height needs no new
    /// canvas code. Heading and Link sections never hold a child -- they
    /// draw immediate-mode.
    fn make_child(&self, cx: &mut Cx, section: &BookSection) -> Option<WidgetRef> {
        match &section.body {
            SectionBody::Prose { document, source } => {
                let child = WidgetRef::new_with_inner(Box::new(cx.with_vm(reading_prose)));
                child
                    .as_markdown_viewer()
                    .install_document(cx, document.clone(), source.clone());
                Some(child)
            }
            SectionBody::Diagram { scene, .. } => {
                let child = WidgetRef::new_with_inner(Box::new(
                    cx.with_vm(crate::canvas::ClassDiagramSurface::script_new_with_default),
                ));
                if let Some(mut canvas) = child.borrow_mut::<crate::canvas::ClassDiagramSurface>() {
                    canvas.set_scene(cx, (**scene).clone());
                    // Read-only embed: interaction stays off in Phase 1.
                    canvas.set_interaction_enabled(cx, false);
                }
                Some(child)
            }
            SectionBody::Heading | SectionBody::Link { .. } => None,
        }
    }

    // Test accessor: which sections currently hold a live child, sorted for
    // a deterministic assertion.
    #[allow(dead_code)] // exercised only by this file's widget tests
    pub(crate) fn live_section_indices(&self) -> Vec<usize> {
        let mut indices: Vec<usize> = self.live.keys().copied().collect();
        indices.sort_unstable();
        indices
    }

    // Test accessor: the live children themselves, sorted by section index
    // -- lets a test assert WHICH widget type backs each kind of section.
    #[cfg(test)]
    pub(crate) fn live_children_for_test(&self) -> Vec<(usize, WidgetRef)> {
        let mut items: Vec<(usize, WidgetRef)> =
            self.live.iter().map(|(i, w)| (*i, w.clone())).collect();
        items.sort_by_key(|(i, _)| *i);
        items
    }

    fn hover_target_at(&self, pos: DVec2) -> Option<BookHoverTarget> {
        // The cached rects are pushed UNCLIPPED (the draw loop culls only
        // fully invisible sections, so a half-scrolled row's rect extends
        // past the viewport into whatever chrome sits above or below it);
        // the viewport clip that bounds what is painted must equally bound
        // what is clickable and hoverable.
        if !self.viewport_rect.contains(pos) {
            return None;
        }
        if let Some(index) = hit_row(&self.link_rects, pos) {
            return Some(BookHoverTarget::Link(index));
        }
        hit_row(&self.open_full_rects, pos).map(BookHoverTarget::OpenFull)
    }

    /// One place the hover state changes: keeps the cursor's
    /// hover_in/hover_out strictly paired and redraws only on a real change.
    fn update_hover(&mut self, cx: &mut Cx, target: Option<BookHoverTarget>) {
        if self.hovered == target {
            return;
        }
        self.hovered = target;
        if target.is_some() {
            crate::cursor::hover_in(cx, MouseCursor::Hand);
        } else {
            crate::cursor::hover_out(cx);
        }
        self.view.redraw(cx);
    }

    fn actions_action(&self, actions: &Actions) -> BookSurfaceAction {
        actions
            .find_widget_action(self.widget_uid())
            .map(|item| item.cast())
            .unwrap_or_default()
    }

    /// The section index a Link row was clicked on this pass, if any.
    /// `BookView::handle` maps it through `navigation_for_section`.
    pub fn link_clicked(&self, actions: &Actions) -> Option<usize> {
        match self.actions_action(actions) {
            BookSurfaceAction::LinkClicked(i) => Some(i),
            _ => None,
        }
    }

    /// The section index a diagram's open-full affordance was clicked on
    /// this pass, if any. Same navigation mapping as `link_clicked`.
    pub fn open_full_clicked(&self, actions: &Actions) -> Option<usize> {
        match self.actions_action(actions) {
            BookSurfaceAction::OpenFullClicked(i) => Some(i),
            _ => None,
        }
    }

    /// The section index a reader-driven scroll (wheel or scrollbar drag)
    /// just crossed into, if any. `BookView::handle` maps it to a tree mark
    /// (`ViewOutcome::tree_mark`), never a navigation.
    pub fn fold_moved(&self, actions: &Actions) -> Option<usize> {
        match self.actions_action(actions) {
            BookSurfaceAction::FoldMoved(i) => Some(i),
            _ => None,
        }
    }

    pub fn scroll_to_section(&mut self, cx: &mut Cx, index: usize) {
        if let Some(&top) = self.tops.get(index) {
            // Clamped like every reader-driven path: a trailing section
            // cannot put its top at the fold without scrolling past the
            // book's end -- blank viewport below the last row, and a
            // scrollbar thumb drawn past its track (`progress > 1.0`) that
            // the next wheel tick would visibly snap back.
            self.set_scroll(top);
            // The reveal moved the fold, so sync the change gate to wherever
            // the fold actually landed (== `index`, except in the clamped
            // trailing case, where it is the section genuinely at the fold)
            // WITHOUT emitting: echoing a mark back at the tree row the
            // reader just clicked would be noise, and leaving the gate stale
            // would make the reader's next small scroll inside the revealed
            // section re-pulse that same row.
            self.last_marked = self.current_section_index();
            self.reconcile_live(cx, self.last_viewport);
            self.view.redraw(cx);
        }
    }

    pub(crate) fn current_section_index(&self) -> Option<usize> {
        crate::book_layout::current_section(&self.tops, self.scroll)
    }

    /// The change gate the reader-scroll paths and the test accessor share:
    /// `Some(index)` exactly when the section at the fold differs from the
    /// last one marked, advancing the marker as it fires. One shared gate is
    /// what makes `FoldMoved` per-crossing, never per scroll tick.
    ///
    /// Deliberately NOT run by `scroll_to_section`: that is the tree->book
    /// direction, and echoing a mark back at the tree row the reader just
    /// clicked would be noise.
    fn fold_crossing(&mut self) -> Option<usize> {
        let current = self.current_section_index();
        if current == self.last_marked {
            return None;
        }
        self.last_marked = current;
        current
    }

    fn emit_fold_crossing(&mut self, cx: &mut Cx) {
        if let Some(index) = self.fold_crossing() {
            cx.widget_action(self.widget_uid(), BookSurfaceAction::FoldMoved(index));
        }
    }

    /// Test probe over the SAME gate `emit_fold_crossing` fires through --
    /// headless tests have no action collection loop to observe `FoldMoved`
    /// on, so they drive the fold with `scroll_to_section` and ask the gate
    /// directly.
    #[cfg(test)]
    pub(crate) fn take_fold_moved_for_test(&mut self) -> bool {
        self.fold_crossing().is_some()
    }

    // Test accessor.
    #[allow(dead_code)]
    pub(crate) fn scroll(&self) -> f64 {
        self.scroll
    }

    fn max_scroll(&self) -> f64 {
        let content: f64 = self.heights.iter().sum();
        (content - self.last_viewport).max(0.0)
    }

    fn set_scroll(&mut self, scroll: f64) {
        self.scroll = scroll.clamp(0.0, self.max_scroll());
    }

    /// Absolute rect of the scroll-bar thumb for the cached viewport rect
    /// and current offset, or `None` when the whole book fits. Mirrors
    /// `TreeLayout::thumb_rect` -- the same rect is painted and hit-tested.
    fn thumb_rect(&self) -> Option<Rect> {
        let rect = self.viewport_rect;
        let content: f64 = self.heights.iter().sum();
        if content <= rect.size.y || rect.size.y <= 0.0 {
            return None;
        }
        let visible = (rect.size.y / content).clamp(0.0, 1.0);
        let thumb_h = (rect.size.y * visible).max(crate::tree_layout::SCROLLBAR_MIN_THUMB);
        let travel = rect.size.y - thumb_h;
        let max_scroll = self.max_scroll();
        let progress = if max_scroll > 0.0 {
            self.scroll / max_scroll
        } else {
            0.0
        };
        Some(Rect {
            pos: dvec2(
                rect.pos.x + rect.size.x - crate::tree_layout::SCROLLBAR_W,
                rect.pos.y + travel * progress,
            ),
            size: dvec2(crate::tree_layout::SCROLLBAR_W, thumb_h),
        })
    }

    /// Invert `thumb_rect`: the (unclamped) scroll offset that places the
    /// thumb's top at absolute `thumb_y`. Mirrors
    /// `TreeLayout::scroll_for_thumb_y`.
    fn scroll_for_thumb_y(&self, thumb_y: f64) -> f64 {
        let rect = self.viewport_rect;
        let content: f64 = self.heights.iter().sum();
        if content <= rect.size.y {
            return 0.0;
        }
        let visible = (rect.size.y / content).clamp(0.0, 1.0);
        let thumb_h = (rect.size.y * visible).max(crate::tree_layout::SCROLLBAR_MIN_THUMB);
        let travel = (rect.size.y - thumb_h).max(1.0);
        let progress = ((thumb_y - rect.pos.y) / travel).clamp(0.0, 1.0);
        progress * self.max_scroll()
    }

    fn draw_heading_row(&mut self, cx: &mut Cx2d, section: &BookSection, rect: Rect) {
        let indent = section.depth as f64 * DEPTH_INDENT;
        let x = (rect.pos.x + HEADING_LEFT + indent).round();
        let y = (rect.pos.y + HEADING_TOP_PAD).round();
        // Steps down with depth, floored so a deeply-nested heading stays
        // legible rather than vanishing.
        let step = (section.depth as f32 * 0.08).min(0.32);
        self.draw_heading.font_scale = 1.0 - step;
        self.draw_heading.draw_abs(cx, dvec2(x, y), &section.title);
        self.draw_heading.font_scale = 1.0;
        self.draw_rule.draw_abs(
            cx,
            Rect {
                pos: dvec2(rect.pos.x, rect.pos.y + rect.size.y - 1.0),
                size: dvec2(rect.size.x, 1.0),
            },
        );
    }

    fn draw_link_row(
        &mut self,
        cx: &mut Cx2d,
        index: usize,
        section: &BookSection,
        reason: &LinkReason,
        rect: Rect,
    ) {
        if self.hovered == Some(BookHoverTarget::Link(index)) {
            self.draw_row_hover.draw_abs(cx, rect);
        }
        let indent = section.depth as f64 * DEPTH_INDENT;
        let icon_x = (rect.pos.x + LINK_LEFT + indent).round();
        let icon_y = (rect.pos.y + (rect.size.y - LINK_ICON_SIZE) / 2.0).round();
        self.icons.draw(
            cx,
            link_icon(reason),
            Rect {
                pos: dvec2(icon_x, icon_y),
                size: dvec2(LINK_ICON_SIZE, LINK_ICON_SIZE),
            },
            self.draw_link_icon.color,
        );
        let text_x = icon_x + LINK_ICON_SIZE + 6.0;
        let size = self
            .draw_link
            .layout(cx, 0.0, 0.0, None, false, Align::default(), &section.title)
            .size_in_lpxs;
        let text_y = (rect.pos.y + (rect.size.y - size.height as f64) / 2.0).round();
        self.draw_link
            .draw_abs(cx, dvec2(text_x, text_y), &section.title);
        // The whole row is clickable, not just the icon/text run -- a
        // generous hit target, and it matches the rect the hover wash
        // painted above.
        self.link_rects.push((index, rect));
    }

    /// The strip above a live diagram embed: the section title left, a
    /// small "open full" affordance right (`Icon::ArrowRight` -- the
    /// catalog has no dedicated external-link glyph yet). Clicking it opens
    /// the concept's own tab through `book_view::navigation_for_section`,
    /// exactly like a Link row; the hit rect is cached the same way.
    fn draw_diagram_caption(
        &mut self,
        cx: &mut Cx2d,
        index: usize,
        section: &BookSection,
        rect: Rect,
    ) {
        if self.hovered == Some(BookHoverTarget::OpenFull(index)) {
            self.draw_row_hover.draw_abs(cx, rect);
        }
        let text_x = (rect.pos.x + HEADING_LEFT).round();
        let size = self
            .draw_caption
            .layout(cx, 0.0, 0.0, None, false, Align::default(), &section.title)
            .size_in_lpxs;
        let text_y = (rect.pos.y + (rect.size.y - size.height as f64) / 2.0).round();
        self.draw_caption
            .draw_abs(cx, dvec2(text_x, text_y), &section.title);

        let button_rect = Rect {
            pos: dvec2(
                rect.pos.x + rect.size.x - OPEN_FULL_ICON_SIZE - LINK_LEFT,
                (rect.pos.y + (rect.size.y - OPEN_FULL_ICON_SIZE) / 2.0).round(),
            ),
            size: dvec2(OPEN_FULL_ICON_SIZE, OPEN_FULL_ICON_SIZE),
        };
        self.icons
            .draw(cx, Icon::ArrowRight, button_rect, self.draw_link_icon.color);
        self.open_full_rects.push((index, button_rect));
    }
}

impl BookSurfaceRef {
    /// See [`BookSurface::link_clicked`].
    pub fn link_clicked(&self, actions: &Actions) -> Option<usize> {
        self.borrow().and_then(|inner| inner.link_clicked(actions))
    }

    /// See [`BookSurface::open_full_clicked`].
    pub fn open_full_clicked(&self, actions: &Actions) -> Option<usize> {
        self.borrow()
            .and_then(|inner| inner.open_full_clicked(actions))
    }

    /// See [`BookSurface::fold_moved`].
    pub fn fold_moved(&self, actions: &Actions) -> Option<usize> {
        self.borrow().and_then(|inner| inner.fold_moved(actions))
    }
}

/// The leading glyph for a degraded (Link) section, chosen from why it
/// degraded -- a nested book reads as a book, everything else as a plain
/// document, matching the tree's own icon vocabulary.
fn link_icon(reason: &LinkReason) -> Icon {
    match reason {
        LinkReason::NestedBook => Icon::Book,
        // An already-inlined folder is still a folder: the row leads to its
        // directory tab, so it wears the tree's folder glyph.
        LinkReason::AlreadyInlined => Icon::Folder,
        LinkReason::UnrenderedSurface(_) | LinkReason::CompileFailed(_) => Icon::FileText,
    }
}

/// The first (index, rect) in `rects` containing `pos`. Shared by hover
/// hit-testing and click hit-testing so both agree on the exact same
/// regions.
/// One themed reading-prose viewer, built from App's `mod.widgets.ReadingProse`
/// alias -- the same object the reading tab mounts, so a book section and a
/// reading tab of the same document typeset identically.
///
/// Not `MarkdownViewer::script_new_with_default`: `MarkdownViewer` lives in
/// `waml-markdown-editor`, below this crate, so its type default carries no
/// palette and the bare widget draws prose in makepad's dark-theme text colour
/// -- invisible on our light surface, and green in every headless test.
/// `reading_prose_alias_is_declared` pins the alias's existence.
///
/// A missing alias falls back to the bare default and says so: pale prose still
/// beats no prose, and the log names the cause rather than leaving a surface
/// that merely looks empty.
fn reading_prose(vm: &mut ScriptVm) -> MarkdownViewer {
    let value = reading_prose_value(vm);
    if value.is_nil() {
        log!("book surface: mod.widgets.ReadingProse is not declared, falling back to the unthemed MarkdownViewer default");
        return MarkdownViewer::script_new_with_default(vm);
    }
    MarkdownViewer::script_from_value(vm, value)
}

/// The raw alias lookup, split out so a test can assert the alias resolves
/// without instantiating a widget.
pub(crate) fn reading_prose_value(vm: &mut ScriptVm) -> ScriptValue {
    use makepad_widgets::makepad_script::trap::NoTrap;
    let widgets = vm.module(id!(widgets));
    vm.bx.heap.value(widgets, id!(ReadingProse).into(), NoTrap)
}

fn hit_row(rects: &[(usize, Rect)], pos: DVec2) -> Option<usize> {
    rects.iter().find(|(_, r)| r.contains(pos)).map(|(i, _)| *i)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::rc::Rc;

    fn model() -> Rc<crate::book_model::BookModel> {
        let mut session = crate::editor_session::EditorSession::default();
        // 40 prose sections: far more than a 600px viewport holds at the
        // 320px prose estimate.
        let mut pairs = vec![("index.md".to_string(), {
            let mut index = String::from("---\nview: book\n---\n# Big\n\n");
            for i in 0..40 {
                index.push_str(&format!("* [S{i}](s{i}.md)\n"));
            }
            index
        })];
        for i in 0..40 {
            pairs.push((format!("s{i}.md"), format!("# S{i}\n\nBody.\n")));
        }
        session
            .replace(
                waml::source::SourceBundle::try_from_pairs(
                    pairs.iter().map(|(a, b)| (a.as_str(), b.as_str())),
                )
                .unwrap(),
            )
            .unwrap();
        Rc::new(
            crate::book_model::build_book(
                &session.snapshot(),
                "/",
                waml::view::chain::ChainLimits::default(),
                &waml::view::mask::ProjectionMask::default(),
            )
            .unwrap(),
        )
    }

    fn surface(cx: &mut Cx) -> BookSurface {
        cx.with_vm(BookSurface::script_new_with_default)
    }

    #[test]
    fn only_viewport_adjacent_sections_hold_live_children() {
        let mut cx = Cx::new(Box::new(|_, _| {}));
        cx.init_cx_os();
        let mut book = surface(&mut cx);
        book.set_model(&mut cx, model());
        book.reconcile_live(&mut cx, 600.0);
        let live = book.live_section_indices();
        assert!(!live.is_empty());
        assert!(
            live.len() < 40,
            "a 40-section book must not instantiate 40 live children, got {live:?}"
        );
        // Scrolled to the bottom, the live window moves with it.
        book.scroll_to_section(&mut cx, 39);
        book.reconcile_live(&mut cx, 600.0);
        let live_at_end = book.live_section_indices();
        assert!(live_at_end.contains(&39));
        assert!(!live_at_end.contains(&0));
    }

    #[test]
    fn scroll_to_section_lands_the_sections_top_at_the_fold() {
        let mut cx = Cx::new(Box::new(|_, _| {}));
        cx.init_cx_os();
        let mut book = surface(&mut cx);
        book.set_model(&mut cx, model());
        // A real viewport, so the end stop exists: this test's promise holds
        // for any section whose top lies BEFORE that stop (here: 5 of 40);
        // a trailing section clamps instead -- pinned separately by
        // `a_reveal_of_a_trailing_section_clamps_to_the_books_end`.
        book.reconcile_live(&mut cx, 600.0);
        book.scroll_to_section(&mut cx, 5);
        assert_eq!(book.current_section_index(), Some(5));
    }

    #[test]
    fn a_reveal_of_a_trailing_section_clamps_to_the_books_end() {
        let mut cx = Cx::new(Box::new(|_, _| {}));
        cx.init_cx_os();
        let mut book = surface(&mut cx);
        book.set_model(&mut cx, model());
        book.reconcile_live(&mut cx, 600.0); // a real viewport, so the end exists
        book.scroll_to_section(&mut cx, 39);
        let content: f64 = book.heights.iter().sum();
        let max = (content - 600.0).max(0.0);
        assert!(
            book.tops[39] > max,
            "fixture: the last section's top lies past the end stop"
        );
        assert_eq!(
            book.scroll(),
            max,
            "the reveal scrolls as far as possible, never past the book's end"
        );
        // The gate synced to the section genuinely at the fold, so the
        // reader's next gate check does not spuriously mark.
        assert!(!book.take_fold_moved_for_test());
    }

    #[test]
    fn a_reveal_syncs_the_fold_gate_without_marking() {
        let mut cx = Cx::new(Box::new(|_, _| {}));
        cx.init_cx_os();
        let mut book = surface(&mut cx);
        book.set_model(&mut cx, model());
        // The reader scrolls and is marked at section 2...
        book.set_scroll(book.tops[2]);
        assert!(book.take_fold_moved_for_test());
        // ...then clicks a tree row: the reveal itself must not mark, and
        // must not leave the gate stale either -- the next gate check (the
        // reader's first scroll tick inside the revealed section) must not
        // re-pulse the row that was just clicked.
        book.scroll_to_section(&mut cx, 10);
        assert_eq!(book.current_section_index(), Some(10));
        assert!(!book.take_fold_moved_for_test(), "no echo after a reveal");
    }

    #[test]
    fn crossing_a_section_boundary_updates_the_current_section_once() {
        let mut cx = Cx::new(Box::new(|_, _| {}));
        cx.init_cx_os();
        let mut book = surface(&mut cx);
        book.set_model(&mut cx, model());
        assert_eq!(book.current_section_index(), Some(0));
        // Reader movement is simulated with the same clamped setter the
        // wheel/scrollbar paths use -- `scroll_to_section` is the tree->book
        // reveal, which deliberately syncs the gate without marking (pinned
        // by `a_reveal_syncs_the_fold_gate_without_marking`).
        book.set_scroll(book.tops[3]);
        assert_eq!(book.current_section_index(), Some(3));
        // The change gate: the SAME index twice must not re-emit. Asserted
        // through the widget's own gate accessor rather than the action
        // queue -- headless tests have no action collection loop.
        assert!(book.take_fold_moved_for_test(), "first crossing marks");
        assert!(
            !book.take_fold_moved_for_test(),
            "no re-mark without movement"
        );
    }

    #[test]
    fn a_model_swap_drops_stale_children_and_clamps_scroll() {
        let mut cx = Cx::new(Box::new(|_, _| {}));
        cx.init_cx_os();
        let mut book = surface(&mut cx);
        book.set_model(&mut cx, model());
        book.scroll_to_section(&mut cx, 39);
        book.set_model(&mut cx, model());
        // A rebuild keyed on the same RowIds keeps the reader's place
        // (the mint/resolve round-trip invariant in root.rs is exactly this
        // promise); scroll is preserved, not reset to zero.
        assert!(book.scroll() > 0.0);
        book.reconcile_live(&mut cx, 600.0);
        assert!(!book.live_section_indices().is_empty());
    }

    /// The prose measurement contract, at the level a headless test can
    /// honestly pin. Covered here: a live prose child reports NO height
    /// before any draw (`content_height` is zero, so the estimate stays
    /// authoritative rather than being clobbered by a phantom measurement),
    /// and a non-zero measurement that differs from the estimate updates the
    /// `RowId`-keyed cache and moves every later section after re-layout.
    /// NOT covered: that `draw_walk`'s Fit-height walk actually makes
    /// `MarkdownViewer::content_height` report the real drawn height -- that
    /// needs a live draw pass, which the headless gate cannot run, and is
    /// owed a visual check.
    #[test]
    fn a_real_measurement_replaces_the_estimate_and_moves_later_sections() {
        let mut cx = Cx::new(Box::new(|_, _| {}));
        cx.init_cx_os();
        let mut book = surface(&mut cx);
        let model = model();
        book.set_model(&mut cx, model.clone());
        book.reconcile_live(&mut cx, 600.0);
        // Before any draw, a live prose child has no measurement to offer...
        let (_, child) = book.live_children_for_test().into_iter().next().unwrap();
        assert_eq!(child.as_markdown_viewer().content_height(), 0.0);
        // ...and the zero sentinel must not disturb the estimate cache.
        assert!(!book.record_measured(&model.sections[0], 0.0));

        let estimate = book.heights[0];
        let next_top_before = book.tops[1];
        let drawn = estimate + 480.0;
        assert!(book.record_measured(&model.sections[0], drawn));
        book.rebuild_layout();
        assert_eq!(book.heights[0], drawn);
        assert!(
            (book.tops[1] - (next_top_before + 480.0)).abs() < 1e-6,
            "every later section moves by the measured delta"
        );
        // Re-reporting the same height is not a change: no re-layout churn.
        assert!(!book.record_measured(&model.sections[0], drawn));
    }

    #[test]
    fn swapping_to_a_different_book_resets_the_reader_state() {
        let mut cx = Cx::new(Box::new(|_, _| {}));
        cx.init_cx_os();
        let mut book = surface(&mut cx);
        book.set_model(&mut cx, model());
        book.scroll_to_section(&mut cx, 39);
        let place = book.scroll();
        assert!(place > 0.0);
        // Same directory: a rebuild keeps the reader's place...
        book.set_model(&mut cx, model());
        assert_eq!(book.scroll(), place);
        // ...and the gate the reveal synced survives it: no mark without
        // fresh reader movement.
        assert!(!book.take_fold_moved_for_test());
        // A DIFFERENT directory: fresh book, top of the page, gate re-armed.
        book.set_model(&mut cx, guide_model());
        assert_eq!(book.scroll(), 0.0, "a different book starts at its top");
        assert_eq!(book.current_section_index(), Some(0));
        book.set_scroll(book.tops[1]);
        assert!(
            book.take_fold_moved_for_test(),
            "the first crossing in a new book must mark"
        );
    }

    /// The Task 2 fixture: a book with a prose section (Intro), a diagram
    /// section (Flow), a nested plain folder (Deep -> Leaf, prose), and a
    /// nested book (Inner, a Link). Duplicated from `book_model.rs`'s
    /// `book_source` -- test modules may not import each other's.
    fn guide_model() -> Rc<crate::book_model::BookModel> {
        let mut session = crate::editor_session::EditorSession::default();
        session
            .replace(
                waml::source::SourceBundle::try_from_pairs([
                    (
                        "index.md",
                        "# Root\n\n* [Guide](guide/)\n* [Loose](loose.md)\n",
                    ),
                    (
                        "guide/index.md",
                        "---\nview: book\n---\n# Guide\n\n* [Intro](intro.md)\n* [Flow](flow.md)\n* [Deep](deep/)\n* [Inner](inner/)\n",
                    ),
                    ("guide/intro.md", "# Intro\n\nSome prose.\n"),
                    (
                        "guide/flow.md",
                        "---\ntype: uml.ClassDiagram\ntitle: Flow\n---\n# Flow\n",
                    ),
                    ("guide/deep/index.md", "# Deep\n\n* [Leaf](leaf.md)\n"),
                    ("guide/deep/leaf.md", "# Leaf\n"),
                    (
                        "guide/inner/index.md",
                        "---\nview: book\n---\n# Inner\n\n* [Nested](nested.md)\n",
                    ),
                    ("guide/inner/nested.md", "# Nested\n"),
                    ("loose.md", "# Loose\n"),
                ])
                .unwrap(),
            )
            .unwrap();
        Rc::new(
            crate::book_model::build_book(
                &session.snapshot(),
                "/guide",
                waml::view::chain::ChainLimits::default(),
                &waml::view::mask::ProjectionMask::default(),
            )
            .unwrap(),
        )
    }

    #[test]
    fn live_children_match_their_section_kind() {
        // Same headless construction as the tests above, over the Task 2
        // fixture (prose + diagram + heading + link). After reconcile_live
        // at the top: the prose sections' children borrow as
        // MarkdownViewer, the diagram section's as ClassDiagramSurface, and
        // Heading/Link sections hold no child (they are immediate-mode
        // rows).
        let mut cx = Cx::new(Box::new(|_, _| {}));
        cx.init_cx_os();
        let mut book = surface(&mut cx);
        book.set_model(&mut cx, guide_model());
        book.reconcile_live(&mut cx, 2000.0);
        let live = book.live_children_for_test();
        let viewer_count = live
            .iter()
            .filter(|(_, w)| {
                w.borrow::<waml_markdown_editor::reading::MarkdownViewer>()
                    .is_some()
            })
            .count();
        let canvas_count = live
            .iter()
            .filter(|(_, w)| w.borrow::<crate::canvas::ClassDiagramSurface>().is_some())
            .count();
        assert!(viewer_count >= 2, "Intro and Leaf are prose");
        assert_eq!(canvas_count, 1, "Flow is the one diagram embed");
    }
}
