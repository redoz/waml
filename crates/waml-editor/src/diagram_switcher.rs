//! Diagram switcher (UX mock): a small chip at the left of the doc-tab
//! strip, showing the current diagram's title. Click cycles the base tab
//! to the next `Model::diagrams` entry (wrapping around), asking `App` to
//! swap it in (`App::switch_diagram`, the same code path the tree panel's
//! diagram row already used before this unit -- both are folded into one
//! shared method).
//!
//! This started as a popover/dropdown design (open a flyout listing every
//! diagram), but that ran into a paint-order problem this immediate-mode
//! codebase doesn't have a clean answer for yet: sibling widgets paint in
//! strict declaration order, so *any* extra content this widget draws
//! outside its own reserved footprint risks being overpainted by whichever
//! sibling is declared after it, or overpainting a sibling declared before
//! it, depending on placement -- and several placements were tried and
//! empirically failed to render/hit-test correctly. Click-to-cycle needs
//! no extra content beyond this widget's own reserved rect, so it sidesteps
//! the whole problem while still exercising the same `switch_diagram` path.
//! A real dropdown is better done with a proper popover/layering primitive,
//! which this codebase doesn't have yet -- worth revisiting later.
//!
//! Hand-rolled immediate-mode widget, same `draw_abs`/rect-hit-test
//! convention as `doc_tabs.rs`/`tool_dock.rs`/`selection_toolbar.rs`.

use makepad_widgets::*;

script_mod! {
    use mod.prelude.widgets_internal.*
    use mod.atlas
    use mod.widgets.*
    use mod.text.*
    use mod.fonts

    mod.widgets.DiagramSwitcherBase = #(DiagramSwitcher::register_widget(vm))

    mod.widgets.DiagramSwitcher = set_type_default() do mod.widgets.DiagramSwitcherBase{
        width: 180.0
        height: Fill
        draw_bg +: { color: atlas.surface }
        draw_edge +: { color: atlas.frame_hi }
        draw_label +: {
            color: atlas.text
            text_style: fonts.text_body
        }
        draw_caret +: {
            color: atlas.text_dim
            text_style: fonts.text_label
        }
    }
}

#[derive(Clone, Debug, Default)]
pub enum DiagramSwitcherAction {
    #[default]
    None,
    Clicked,
}

/// Pick the diagram key to switch to when the chip is clicked: the entry
/// right after `current` in `keys`, wrapping to the first; if `current`
/// isn't found (or `keys` is empty), falls back to the first entry.
/// Pure so it's unit-tested without a `Cx`.
pub fn next_diagram_key(keys: &[String], current: &str) -> Option<String> {
    if keys.is_empty() {
        return None;
    }
    match keys.iter().position(|k| k == current) {
        Some(i) => Some(keys[(i + 1) % keys.len()].clone()),
        None => Some(keys[0].clone()),
    }
}

#[derive(Script, ScriptHook, Widget)]
pub struct DiagramSwitcher {
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
    draw_label: DrawText,
    #[redraw]
    #[live]
    draw_caret: DrawText,

    #[rust]
    current_title: String,
}

impl Widget for DiagramSwitcher {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, _scope: &mut Scope) {
        let uid = self.widget_uid();
        match event.hits_with_capture_overload(cx, self.draw_bg.area(), true) {
            Hit::FingerUp(fe) if fe.is_primary_hit() => {
                cx.widget_action(uid, DiagramSwitcherAction::Clicked);
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

        let text_y = rect.pos.y + rect.size.y * 0.5 - 6.0;
        self.draw_label
            .draw_abs(cx, dvec2(rect.pos.x + 10.0, text_y), &self.current_title);
        // Plain ASCII caret -- unicode glyphs render as tofu with the sole
        // vendored font, per the same finding recorded in `tool_dock.rs`.
        self.draw_caret
            .draw_abs(cx, dvec2(rect.pos.x + rect.size.x - 16.0, text_y), ">");

        DrawStep::done()
    }
}

impl DiagramSwitcher {
    /// Update the trigger chip's label to the currently-loaded diagram's
    /// title. Called wherever the base tab's diagram changes.
    pub fn set_current(&mut self, cx: &mut Cx, title: &str) {
        self.current_title = title.to_string();
        self.draw_bg.redraw(cx);
    }

    /// Convenience reader for `App`, mirroring `DocTabs::tab_action`.
    pub fn switcher_action(&self, actions: &Actions) -> Option<DiagramSwitcherAction> {
        let item = actions.find_widget_action(self.widget_uid())?;
        match item.cast() {
            DiagramSwitcherAction::None => None,
            action => Some(action),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cycles_to_next_key() {
        let keys = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        assert_eq!(next_diagram_key(&keys, "a"), Some("b".to_string()));
        assert_eq!(next_diagram_key(&keys, "c"), Some("a".to_string()));
    }

    #[test]
    fn falls_back_to_first_when_current_unknown() {
        let keys = vec!["a".to_string(), "b".to_string()];
        assert_eq!(next_diagram_key(&keys, "zzz"), Some("a".to_string()));
    }

    #[test]
    fn empty_keys_yields_none() {
        assert_eq!(next_diagram_key(&[], "a"), None);
    }
}
