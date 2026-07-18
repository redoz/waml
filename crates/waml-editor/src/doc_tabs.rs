//! Doc tab strip: a permanent "Diagram" tab plus Zed-style preview/persisted
//! classifier tabs. `OpenTabs` is pure state (no `Cx`), unit-tested like
//! `tree.rs`/`inspector.rs`. `DocTabs` is the immediate-mode widget that
//! renders it as a hand-rolled `DrawText` strip — no fork `TabBar` machinery,
//! same convention as `GraphCanvas`/`inspector_panel` (`draw_abs` at manually
//! tracked positions, click regions captured during `draw_walk` and hit-tested
//! against on `FingerUp`).

use makepad_widgets::*;

script_mod! {
    use mod.prelude.widgets_internal.*
    use mod.atlas
    use mod.widgets.*
    use mod.text.*

    mod.widgets.DocTabsBase = #(DocTabs::register_widget(vm))

    mod.widgets.DocTabs = set_type_default() do mod.widgets.DocTabsBase{
        width: Fill
        height: 34.0
        draw_bg +: { color: atlas.surface }
        draw_edge +: { color: atlas.frame_hi }
        draw_tab_active +: { color: atlas.selection }
        draw_text_active +: {
            color: atlas.text
            text_style: TextStyle{
                font_size: 12
                font_family: FontFamily{
                    latin := FontMember{res: crate_resource("self:resources/fonts/IBMPlexSans-Regular.ttf") asc: -0.1 desc: 0.0}
                }
                line_spacing: 1.2
            }
        }
        draw_text_persisted +: {
            color: atlas.text_dim
            text_style: TextStyle{
                font_size: 12
                font_family: FontFamily{
                    latin := FontMember{res: crate_resource("self:resources/fonts/IBMPlexSans-Regular.ttf") asc: -0.1 desc: 0.0}
                }
                line_spacing: 1.2
            }
        }
        draw_text_preview +: {
            color: atlas.text_dim
            text_style: TextStyle{
                font_size: 12
                font_family: FontFamily{
                    latin := FontMember{res: crate_resource("self:resources/fonts/IBMPlexSans-Regular.ttf") asc: -0.1 desc: 0.0}
                }
                line_spacing: 1.2
            }
        }
        draw_close +: {
            color: atlas.text_dim
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

/// What a tab points at.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TabKind {
    Diagram,
    Classifier,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocTab {
    pub id: LiveId,
    pub key: String,
    pub title: String,
    pub kind: TabKind,
    /// A preview tab is replaced in place by the next classifier click; an
    /// inline-edit commit "pins" it (`promote`), after which it behaves like
    /// any other persisted tab.
    pub preview: bool,
}

/// The open-tabs state: `tabs[0]` is always the permanent Diagram tab
/// (`preview: false`, never closable).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenTabs {
    pub tabs: Vec<DocTab>,
    pub active: LiveId,
}

/// The pre-startup default: no tabs at all. `App::handle_startup` immediately
/// replaces this with `OpenTabs::diagram_base(..)` once the model is loaded.
impl Default for OpenTabs {
    fn default() -> Self {
        OpenTabs {
            tabs: vec![],
            active: LiveId::default(),
        }
    }
}

impl OpenTabs {
    /// Seed with just the permanent Diagram tab, active.
    pub fn diagram_base(key: impl Into<String>, title: impl Into<String>) -> OpenTabs {
        let key = key.into();
        let id = diagram_tab_id();
        let tab = DocTab {
            id,
            key,
            title: title.into(),
            kind: TabKind::Diagram,
            preview: false,
        };
        OpenTabs {
            active: id,
            tabs: vec![tab],
        }
    }

    fn preview_index(&self) -> Option<usize> {
        self.tabs.iter().position(|t| t.preview)
    }

    /// A classifier single-click: replace the single preview slot in place
    /// (never duplicates, never piles up), or insert one right after the base
    /// if none exists yet. Always activates the resulting tab.
    pub fn open_preview(&mut self, key: impl Into<String>, title: impl Into<String>) -> LiveId {
        let key = key.into();
        let title = title.into();
        let id = classifier_tab_id(&key);
        if let Some(idx) = self.preview_index() {
            self.tabs[idx] = DocTab {
                id,
                key,
                title,
                kind: TabKind::Classifier,
                preview: true,
            };
        } else {
            // No preview slot: append at the end, after any persisted tabs
            // (matches editors that always open new tabs rightmost).
            self.tabs.push(DocTab {
                id,
                key,
                title,
                kind: TabKind::Classifier,
                preview: true,
            });
        }
        self.active = id;
        id
    }

    /// Flip a preview tab to persisted. Idempotent; a no-op for unknown ids.
    pub fn promote(&mut self, id: LiveId) {
        if let Some(tab) = self.tabs.iter_mut().find(|t| t.id == id) {
            tab.preview = false;
        }
    }

    /// Remove a tab. The Diagram base refuses. If the closed tab was active,
    /// activate the right-adjacent tab, else the left, else the base.
    pub fn close(&mut self, id: LiveId) {
        if self.tabs.first().map(|t| t.id) == Some(id) {
            return;
        }
        let Some(idx) = self.tabs.iter().position(|t| t.id == id) else {
            return;
        };
        self.tabs.remove(idx);
        if self.active == id {
            let new_idx = if idx < self.tabs.len() {
                idx
            } else {
                idx.saturating_sub(1)
            };
            self.active = self
                .tabs
                .get(new_idx)
                .map(|t| t.id)
                .unwrap_or(self.tabs[0].id);
        }
    }

    pub fn activate(&mut self, id: LiveId) {
        if self.tabs.iter().any(|t| t.id == id) {
            self.active = id;
        }
    }

    pub fn active_tab(&self) -> Option<&DocTab> {
        self.tabs.iter().find(|t| t.id == self.active)
    }
}

/// The Diagram base tab's id is stable (independent of which diagram is
/// loaded — there is only ever one base tab).
pub fn diagram_tab_id() -> LiveId {
    LiveId::from_str("__doc_tab_diagram__")
}

/// A classifier tab's id is derived from its key so re-previewing the same
/// classifier reuses the same id.
pub fn classifier_tab_id(key: &str) -> LiveId {
    LiveId::from_str(&format!("__doc_tab_classifier__{key}"))
}

// ---------------------------------------------------------------------------
// Widget
// ---------------------------------------------------------------------------

const TAB_W_BASE: f64 = 120.0;
const TAB_W: f64 = 160.0;
const CLOSE_W: f64 = 22.0;
const TEXT_PAD: f64 = 12.0;
const MAX_TITLE_CHARS: usize = 18;

fn truncate_title(s: &str) -> String {
    if s.chars().count() <= MAX_TITLE_CHARS {
        return s.to_string();
    }
    let mut out: String = s.chars().take(MAX_TITLE_CHARS.saturating_sub(1)).collect();
    out.push('…');
    out
}

#[derive(Clone, Debug, Default)]
pub enum DocTabsAction {
    #[default]
    None,
    Activate(LiveId),
    Close(LiveId),
}

#[derive(Script, ScriptHook, Widget)]
pub struct DocTabs {
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
    /// Subtle source-bright top edge (shared HUD panel material).
    #[redraw]
    #[live]
    draw_edge: DrawColor,
    #[redraw]
    #[live]
    draw_tab_active: DrawColor,
    #[redraw]
    #[live]
    draw_text_active: DrawText,
    #[redraw]
    #[live]
    draw_text_persisted: DrawText,
    #[redraw]
    #[live]
    draw_text_preview: DrawText,
    #[redraw]
    #[live]
    draw_close: DrawText,

    #[rust]
    tabs: Vec<DocTab>,
    #[rust]
    active: LiveId,
    #[rust]
    tab_rects: Vec<(LiveId, Rect)>,
    #[rust]
    close_rects: Vec<(LiveId, Rect)>,
}

impl Widget for DocTabs {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, _scope: &mut Scope) {
        let uid = self.widget_uid();
        match event.hits_with_capture_overload(cx, self.draw_bg.area(), true) {
            Hit::FingerUp(fe) if fe.is_primary_hit() => {
                for (id, rect) in self.close_rects.iter().rev() {
                    if rect.contains(fe.abs) {
                        cx.widget_action(uid, DocTabsAction::Close(*id));
                        return;
                    }
                }
                for (id, rect) in self.tab_rects.iter().rev() {
                    if rect.contains(fe.abs) {
                        cx.widget_action(uid, DocTabsAction::Activate(*id));
                        return;
                    }
                }
            }
            Hit::FingerHoverIn(_) => cx.set_cursor(MouseCursor::Hand),
            _ => {}
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, _scope: &mut Scope, walk: Walk) -> DrawStep {
        let rect = cx.walk_turtle(walk);
        self.draw_bg.draw_abs(cx, rect);
        self.draw_edge.draw_abs(
            cx,
            Rect {
                pos: rect.pos,
                size: dvec2(rect.size.x, 1.5),
            },
        );

        self.tab_rects.clear();
        self.close_rects.clear();

        let mut x = rect.pos.x;
        for (i, tab) in self.tabs.iter().enumerate() {
            let closable = tab.kind != TabKind::Diagram;
            let w = if i == 0 { TAB_W_BASE } else { TAB_W };
            let tab_rect = Rect {
                pos: dvec2(x, rect.pos.y),
                size: dvec2(w, rect.size.y),
            };
            let is_active = tab.id == self.active;

            if is_active {
                self.draw_tab_active.draw_abs(cx, tab_rect);
            }

            let text_y = rect.pos.y + rect.size.y * 0.5 - 7.0;
            let title = truncate_title(&tab.title);
            let draw_text = if is_active {
                &mut self.draw_text_active
            } else if tab.preview {
                &mut self.draw_text_preview
            } else {
                &mut self.draw_text_persisted
            };
            draw_text.draw_abs(cx, dvec2(x + TEXT_PAD, text_y), &title);

            if closable {
                let close_rect = Rect {
                    pos: dvec2(x + w - CLOSE_W, rect.pos.y),
                    size: dvec2(CLOSE_W, rect.size.y),
                };
                self.draw_close
                    .draw_abs(cx, dvec2(close_rect.pos.x + 4.0, text_y), "\u{d7}");
                self.close_rects.push((tab.id, close_rect));
            }

            self.tab_rects.push((tab.id, tab_rect));
            x += w;
        }

        DrawStep::done()
    }
}

impl DocTabs {
    pub fn set_tabs(&mut self, cx: &mut Cx, open: &OpenTabs) {
        self.tabs = open.tabs.clone();
        self.active = open.active;
        self.draw_bg.redraw(cx);
    }
}

impl DocTabs {
    /// Convenience reader for `App`, mirroring `ProjectTree::selected_diagram`.
    pub fn tab_action(&self, actions: &Actions) -> Option<DocTabsAction> {
        let item = actions.find_widget_action(self.widget_uid())?;
        match item.cast() {
            DocTabsAction::None => None,
            action => Some(action),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diagram_base_seeds_a_single_active_permanent_tab() {
        let open = OpenTabs::diagram_base("orders-diagram", "Orders");
        assert_eq!(open.tabs.len(), 1);
        assert_eq!(open.tabs[0].kind, TabKind::Diagram);
        assert!(!open.tabs[0].preview);
        assert_eq!(open.active, open.tabs[0].id);
    }

    #[test]
    fn open_preview_twice_replaces_the_single_preview_slot() {
        let mut open = OpenTabs::diagram_base("d", "Diagram");
        open.open_preview("customer", "Customer");
        assert_eq!(open.tabs.len(), 2);
        assert!(open.tabs[1].preview);
        assert_eq!(open.active, open.tabs[1].id);

        open.open_preview("order", "Order");
        // Still base + one preview -- never piles up.
        assert_eq!(open.tabs.len(), 2);
        assert_eq!(open.tabs[1].key, "order");
        assert!(open.tabs[1].preview);
        assert_eq!(open.active, open.tabs[1].id);
    }

    #[test]
    fn promote_then_open_preview_keeps_the_promoted_tab_and_adds_a_fresh_preview() {
        let mut open = OpenTabs::diagram_base("d", "Diagram");
        let customer_id = open.open_preview("customer", "Customer");
        open.promote(customer_id);
        open.open_preview("order", "Order");

        assert_eq!(open.tabs.len(), 3);
        assert_eq!(open.tabs[1].key, "customer");
        assert!(!open.tabs[1].preview, "promoted tab stays persisted");
        assert_eq!(open.tabs[2].key, "order");
        assert!(open.tabs[2].preview);
    }

    #[test]
    fn promote_is_idempotent() {
        let mut open = OpenTabs::diagram_base("d", "Diagram");
        let id = open.open_preview("customer", "Customer");
        open.promote(id);
        open.promote(id);
        assert!(!open.tabs[1].preview);
    }

    #[test]
    fn close_activates_right_adjacent_then_left_then_base() {
        let mut open = OpenTabs::diagram_base("d", "Diagram");
        let a = open.open_preview("a", "A");
        open.promote(a);
        let b = open.open_preview("b", "B");
        open.promote(b);
        let c = open.open_preview("c", "C");
        open.promote(c);
        // tabs: [base, a, b, c], active = c

        open.activate(b);
        open.close(b);
        // b removed; right-adjacent (c) becomes active.
        assert_eq!(open.tabs.len(), 3);
        assert_eq!(open.active, c);

        open.close(c);
        // c was rightmost; falls back to left-adjacent (a).
        assert_eq!(open.tabs.len(), 2);
        assert_eq!(open.active, a);

        open.close(a);
        // a was rightmost now; falls back to the base.
        assert_eq!(open.tabs.len(), 1);
        assert_eq!(open.active, open.tabs[0].id);
    }

    #[test]
    fn close_refuses_the_diagram_base() {
        let mut open = OpenTabs::diagram_base("d", "Diagram");
        let base_id = open.tabs[0].id;
        open.close(base_id);
        assert_eq!(open.tabs.len(), 1, "base tab must survive close");
    }

    #[test]
    fn activate_unknown_id_is_a_no_op() {
        let mut open = OpenTabs::diagram_base("d", "Diagram");
        let before = open.active;
        open.activate(LiveId::from_str("nope"));
        assert_eq!(open.active, before);
    }
}
