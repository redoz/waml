//! The book's one drawing surface: a virtualized vertical scroll of
//! sections. Owns its scroll offset (like `tree_panel`'s `TreeLayout`)
//! because a `View`-owned scroll cannot drive the live-child window;
//! sections draw at fixed or Fit heights, NEVER Fill -- a `draw_walk` rect
//! goes stale after a `Size::Fill` sibling, which would corrupt the
//! measured-height cache.
//!
//! Only Prose/Diagram sections hold a live child widget (a bare placeholder
//! `View` here; Task 6 swaps it for the real `MarkdownViewer`/
//! `ClassDiagramSurface`). Heading and Link sections draw immediate-mode,
//! the same hand-drawn style `tree_row_draw.rs` uses for tree rows, and are
//! positioned from this widget's own `heights`/`tops` -- not from the
//! turtle -- so the offset math stays in one place (`book_layout.rs`).

use std::collections::HashMap;
use std::rc::Rc;

use makepad_widgets::*;

use crate::book_model::{BookModel, BookSection, LinkReason, SectionBody};
use crate::icons::{Icon, IconSet};
use waml::view::row::RowId;

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
        // RGBA crosses Rust.
        draw_link_icon +: { color: atlas.text_dim }
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
}

impl Widget for BookSurface {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);

        // Hand-drawn scroll bar, the exact `tree_panel.rs` drag shape: a
        // primary press on the thumb starts a drag tracked by the pointer's
        // offset from the thumb top, a move maps the pointer back to a
        // scroll offset, and the release ends it. Each returns early so the
        // same press is not also read as a wheel/row event below.
        match event {
            Event::MouseDown(e) if e.button.is_primary() => {
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

        // Wheel/trackpad scroll. This widget owns the offset and clamps it,
        // so a fling past either end simply stops rather than stranding the
        // sections.
        if let Hit::FingerScroll(fe) = event.hits_with_capture_overload(cx, self.view.area(), true)
        {
            let before = self.scroll;
            self.set_scroll(before + fe.scroll.y);
            if self.scroll != before {
                self.reconcile_live(cx, self.last_viewport);
                self.view.redraw(cx);
            }
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        while self.view.draw_walk(cx, scope, walk).step().is_some() {}
        let rect = self.view.area().rect(cx);
        self.viewport_rect = rect;
        self.reconcile_live(cx, rect.size.y);

        cx.push_clip_rect(rect);

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
                        self.draw_link_row(cx, section, reason, section_rect)
                    }
                    SectionBody::Prose { .. } | SectionBody::Diagram { .. } => {
                        if let Some(child) = self.live.get(&index) {
                            let _ = child.draw_walk(cx, scope, Walk::abs_rect(section_rect));
                            let drawn = child.area().rect(cx).size.y;
                            if drawn > 0.0
                                && self.measured.get(&section.row_id).copied() != Some(drawn)
                            {
                                self.measured.insert(section.row_id.clone(), drawn);
                                remeasured = true;
                            }
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
        self.model = Some(model);
        self.rebuild_layout();
        let total: f64 = self.heights.iter().sum();
        self.scroll = self.scroll.min((total - self.last_viewport).max(0.0));
        self.view.redraw(cx);
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

    /// The shell holds a PLACEHOLDER child per Prose/Diagram section (a bare
    /// `View`) so the virtualization tests exercise real child lifecycle
    /// now; Task 6 swaps the placeholders for `MarkdownViewer`/
    /// `ClassDiagramSurface`. Heading and Link sections never hold a child
    /// -- they draw immediate-mode.
    fn make_child(&self, cx: &mut Cx, section: &BookSection) -> Option<WidgetRef> {
        match &section.body {
            SectionBody::Prose { .. } | SectionBody::Diagram { .. } => Some(
                WidgetRef::new_with_inner(Box::new(cx.with_vm(View::script_new_with_default))),
            ),
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

    // Consumed by Task 7 (a tree click reveals a book section), which
    // removes this allow.
    #[allow(dead_code)]
    pub fn scroll_to_section(&mut self, cx: &mut Cx, index: usize) {
        if let Some(&top) = self.tops.get(index) {
            self.scroll = top;
            self.reconcile_live(cx, self.last_viewport);
            self.view.redraw(cx);
        }
    }

    // Consumed by Task 8 (scrolling the book marks the current tree row),
    // which removes this allow.
    #[allow(dead_code)]
    pub(crate) fn current_section_index(&self) -> Option<usize> {
        crate::book_layout::current_section(&self.tops, self.scroll)
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
        section: &BookSection,
        reason: &LinkReason,
        rect: Rect,
    ) {
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
    }
}

/// The leading glyph for a degraded (Link) section, chosen from why it
/// degraded -- a nested book reads as a book, everything else as a plain
/// document, matching the tree's own icon vocabulary.
fn link_icon(reason: &LinkReason) -> Icon {
    match reason {
        LinkReason::NestedBook => Icon::Book,
        LinkReason::UnrenderedSurface(_) | LinkReason::CompileFailed(_) => Icon::FileText,
    }
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
        book.scroll_to_section(&mut cx, 5);
        assert_eq!(book.current_section_index(), Some(5));
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
}
