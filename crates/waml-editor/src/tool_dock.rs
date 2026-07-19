//! Left tool dock (UX mock): a vertical icon strip mirroring the web
//! frontend's toolbox. `Select`/`Add`/`Connect` are the exclusive active
//! tools (mouse click or hotkey V/N/C); `AutoLayout`/`DiagramProps`/
//! `Shortcuts`/`Clear` are one-shot action buttons (no persistent state).
//! Hand-rolled immediate-mode widget, same convention as `doc_tabs.rs`.
//! No tool behavior is wired into the canvas yet -- selecting a tool only
//! changes the dock's own highlight (breadth mock, not polish).

use makepad_widgets::*;

script_mod! {
    use mod.prelude.widgets_internal.*
    use mod.atlas
    use mod.widgets.*
    use mod.text.*

    mod.widgets.ToolDockBase = #(ToolDock::register_widget(vm))

    mod.widgets.ToolDock = set_type_default() do mod.widgets.ToolDockBase{
        width: 48.0
        height: Fill
        draw_bg: mod.draw.AccentFrame{ color: atlas.field_bg }
        draw_edge +: { color: atlas.frame_hi }
        draw_item_active +: { color: atlas.selection }
        draw_glyph_active +: {
            color: atlas.accent
            text_style: TextStyle{
                font_size: 16
                font_family: FontFamily{
                    latin := FontMember{res: crate_resource("self:resources/fonts/IBM_Plex_Sans/IBMPlexSans-Regular.ttf") asc: -0.1 desc: 0.0}
                }
                line_spacing: 1.2
            }
        }
        draw_glyph_dim +: {
            color: atlas.text_dim
            text_style: TextStyle{
                font_size: 16
                font_family: FontFamily{
                    latin := FontMember{res: crate_resource("self:resources/fonts/IBM_Plex_Sans/IBMPlexSans-Regular.ttf") asc: -0.1 desc: 0.0}
                }
                line_spacing: 1.2
            }
        }
        draw_hint +: {
            color: atlas.text_dim
            text_style: TextStyle{
                font_size: 9
                font_family: FontFamily{
                    latin := FontMember{res: crate_resource("self:resources/fonts/IBM_Plex_Sans/IBMPlexSans-Regular.ttf") asc: -0.1 desc: 0.0}
                }
                line_spacing: 1.2
            }
        }
    }
}

/// A tool-dock entry. `Select`/`Add`/`Connect` are mutually-exclusive
/// "modes"; the rest are one-shot actions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Tool {
    #[default]
    Select,
    Add,
    Connect,
    AutoLayout,
    DiagramProps,
    Shortcuts,
    Clear,
}

impl Tool {
    pub const ALL: [Tool; 7] = [
        Tool::Select,
        Tool::Add,
        Tool::Connect,
        Tool::AutoLayout,
        Tool::DiagramProps,
        Tool::Shortcuts,
        Tool::Clear,
    ];

    /// Whether this entry is a persistent mode (highlighted while active)
    /// vs. a one-shot action button.
    pub fn is_mode(self) -> bool {
        matches!(self, Tool::Select | Tool::Add | Tool::Connect)
    }

    /// Glyph drawn in the strip. No icon font is vendored yet, so these are
    /// plain text/unicode stand-ins.
    // Glyphs are plain Latin characters only -- IBM Plex Sans (the sole
    // vendored font) doesn't cover most pictographic/dingbat unicode
    // ranges, which render as tofu boxes. `\u{d7}` (multiplication sign)
    // is the one confirmed-working symbol outside ASCII (already used for
    // the doc-tab close button).
    pub fn glyph(self) -> &'static str {
        match self {
            Tool::Select => "\u{2196}",
            Tool::Add => "+",
            Tool::Connect => "\u{2197}",
            Tool::AutoLayout => "A",
            Tool::DiagramProps => "P",
            Tool::Shortcuts => "?",
            Tool::Clear => "\u{d7}",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Tool::Select => "Select",
            Tool::Add => "Add",
            Tool::Connect => "Connect",
            Tool::AutoLayout => "Auto Layout",
            Tool::DiagramProps => "Diagram Properties",
            Tool::Shortcuts => "Shortcuts",
            Tool::Clear => "Clear Selection",
        }
    }

    /// Single-letter hotkey hint drawn under the glyph. `None` for entries
    /// with no keyboard shortcut in this mock.
    pub fn hotkey_hint(self) -> Option<&'static str> {
        match self {
            Tool::Select => Some("V"),
            Tool::Add => Some("N"),
            Tool::Connect => Some("C"),
            _ => None,
        }
    }
}

/// Map a hotkey letter to the mode it switches to. Pure so it's testable
/// without a `Cx`; the widget/App layer decides *when* to apply it (e.g.
/// only while nothing else holds key focus).
pub fn tool_for_hotkey(letter: char) -> Option<Tool> {
    match letter.to_ascii_uppercase() {
        'V' => Some(Tool::Select),
        'N' => Some(Tool::Add),
        'C' => Some(Tool::Connect),
        _ => None,
    }
}

#[derive(Clone, Debug, Default)]
pub enum ToolDockAction {
    #[default]
    None,
    /// A mode (`Select`/`Add`/`Connect`) became active. Carries the new mode for
    /// callers that want it; today's only listener (`sync_statusbar`) re-reads
    /// the mode from `self` instead, so this field is intentionally unread here.
    ModeChanged(#[allow(dead_code)] Tool),
    /// A one-shot action button was clicked.
    Triggered(Tool),
}

const ITEM_H: f64 = 44.0;
const GLYPH_Y: f64 = 8.0;
const HINT_Y: f64 = 28.0;
const GROUP_GAP: f64 = 10.0;

#[derive(Script, ScriptHook, Widget)]
pub struct ToolDock {
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
    draw_item_active: DrawColor,
    #[redraw]
    #[live]
    draw_glyph_active: DrawText,
    #[redraw]
    #[live]
    draw_glyph_dim: DrawText,
    #[redraw]
    #[live]
    draw_hint: DrawText,

    #[rust]
    active: Tool,
    #[rust]
    item_rects: Vec<(Tool, Rect)>,
}

impl Widget for ToolDock {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, _scope: &mut Scope) {
        let uid = self.widget_uid();
        match event.hits_with_capture_overload(cx, self.draw_bg.area(), true) {
            Hit::FingerUp(fe) if fe.is_primary_hit() => {
                for (tool, rect) in self.item_rects.clone() {
                    if rect.contains(fe.abs) {
                        if tool.is_mode() {
                            self.active = tool;
                            self.draw_bg.redraw(cx);
                            cx.widget_action(uid, ToolDockAction::ModeChanged(tool));
                        } else {
                            cx.widget_action(uid, ToolDockAction::Triggered(tool));
                        }
                        break;
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
        self.item_rects.clear();

        let mut y = rect.pos.y;
        for (i, tool) in Tool::ALL.iter().copied().enumerate() {
            // A gap after the mode group (Select/Add/Connect) separates it
            // visually from the action buttons.
            if i == 3 {
                y += GROUP_GAP;
            }
            let item_rect = Rect {
                pos: dvec2(rect.pos.x, y),
                size: dvec2(rect.size.x, ITEM_H),
            };
            let is_active = tool.is_mode() && tool == self.active;
            if is_active {
                self.draw_item_active.draw_abs(cx, item_rect);
            }

            let glyph_x = rect.pos.x + rect.size.x * 0.5 - 5.0;
            let draw_glyph = if is_active {
                &mut self.draw_glyph_active
            } else {
                &mut self.draw_glyph_dim
            };
            draw_glyph.draw_abs(cx, dvec2(glyph_x, item_rect.pos.y + GLYPH_Y), tool.glyph());

            if let Some(hint) = tool.hotkey_hint() {
                let hint_x = rect.pos.x + rect.size.x * 0.5 - 3.0;
                self.draw_hint
                    .draw_abs(cx, dvec2(hint_x, item_rect.pos.y + HINT_Y), hint);
            }

            self.item_rects.push((tool, item_rect));
            y += ITEM_H;
        }

        DrawStep::done()
    }
}

impl ToolDock {
    /// Set the active mode directly (used by `App` for hotkey-driven
    /// switches, bypassing the click/action round-trip).
    pub fn set_active(&mut self, cx: &mut Cx, tool: Tool) {
        if tool.is_mode() {
            self.active = tool;
            self.draw_bg.redraw(cx);
        }
    }

    pub fn active(&self) -> Tool {
        self.active
    }

    /// Convenience reader for `App`, mirroring `DocTabs::tab_action`.
    pub fn dock_action(&self, actions: &Actions) -> Option<ToolDockAction> {
        let item = actions.find_widget_action(self.widget_uid())?;
        match item.cast() {
            ToolDockAction::None => None,
            action => Some(action),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_active_tool_is_select() {
        assert_eq!(Tool::default(), Tool::Select);
    }

    #[test]
    fn hotkeys_map_to_the_three_modes() {
        assert_eq!(tool_for_hotkey('v'), Some(Tool::Select));
        assert_eq!(tool_for_hotkey('V'), Some(Tool::Select));
        assert_eq!(tool_for_hotkey('n'), Some(Tool::Add));
        assert_eq!(tool_for_hotkey('c'), Some(Tool::Connect));
        assert_eq!(tool_for_hotkey('x'), None);
    }

    #[test]
    fn only_the_first_three_tools_are_modes() {
        for (i, tool) in Tool::ALL.iter().enumerate() {
            assert_eq!(tool.is_mode(), i < 3, "{tool:?} mode-ness mismatch");
        }
    }
}
