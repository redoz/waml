//! Left tool dock (UX mock): a vertical icon strip mirroring the web
//! frontend's toolbox. `Select`/`Add`/`Connect` are the exclusive active
//! tools (mouse click or hotkey V/N/C); `DiagramProps`/`Clear` are one-shot
//! action buttons (no persistent state). No tool behavior is wired into the
//! canvas yet -- selecting a tool only changes the dock's own highlight
//! (breadth mock, not polish).
//!
//! Each entry is a shared [`IconButton`] child, so a dock button reads
//! identically to every other icon button in the app (caption Save/Menu, the
//! inspector's fold/pin): a catalog glyph over a rounded accent wash, lit while
//! hovered or (for the active mode) selected. The dock is a `#[deref] View`
//! that lays out five `IconButton`s in a `flow: Down` strip; `draw_walk` syncs
//! each child's glyph + lit state from `active`, and `handle_event` reads each
//! child's `clicked` action to drive the mode/trigger dispatch. The strip's own
//! `draw_bg` paints the Atlas HUD frame; `draw_edge` reinforces its top edge.

use makepad_widgets::*;

use crate::icon_button::IconButtonWidgetRefExt;
use crate::icons::Icon;

script_mod! {
    use mod.prelude.widgets_internal.*
    use mod.atlas
    use mod.widgets.*

    mod.widgets.ToolDockBase = #(ToolDock::register_widget(vm))

    mod.widgets.ToolDock = set_type_default() do mod.widgets.ToolDockBase{
        width: 48.0
        height: Fill
        flow: Down
        // Centre the 32px buttons in the 48-wide strip; pack from the top.
        align: Align{x: 0.5, y: 0.0}
        padding: Inset{top: 8.0}
        // ~12px between buttons re-creates the old 44px vertical pitch (32px
        // button + 12px gap); `props_btn`'s extra top margin is the group gap.
        spacing: 12.0
        show_bg: true
        // The strip carries the Atlas HUD frame -- the AccentFrame material
        // inlined onto the View's `draw_bg` (keep in sync with `frame.rs` /
        // `inspector_panel.rs`): a `field_bg` fill ringed by the source-bright
        // accent stroke fading along a 150deg diagonal.
        draw_bg +: {
            color: atlas.field_bg
            border_hi: uniform(atlas.frame_hi)
            border_lo: uniform(atlas.frame_lo)
            pixel: fn() {
                let inset = 1.5
                let sdf = Sdf2d.viewport(self.pos * self.rect_size)
                sdf.rect(inset, inset, self.rect_size.x - inset * 2.0, self.rect_size.y - inset * 2.0)
                sdf.fill_keep(self.color)
                let dir = vec2(0.5, 0.8660254)
                let span = 1.3660254
                let t = clamp((self.pos.x * dir.x + self.pos.y * dir.y) / span, 0.0, 1.0)
                sdf.stroke(mix(self.border_hi, self.border_lo, t), inset)
                return sdf.result
            }
        }
        // Subtle source-bright top edge (shared HUD panel material), drawn over
        // the frame's own top border in `draw_walk`.
        draw_edge +: { color: atlas.frame_hi }

        // The five entries: the mode group (Select/Add/Connect) then the action
        // group (DiagramProps/Clear), separated by the group gap. Each is a bare
        // `IconButton` -- its glyph + lit state are pushed per draw from `active`.
        select_btn := IconButton {}
        add_btn := IconButton {}
        connect_btn := IconButton {}
        props_btn := IconButton { margin: Inset{top: 10.0} }
        clear_btn := IconButton {}
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
    DiagramProps,
    Clear,
}

impl Tool {
    pub const ALL: [Tool; 5] = [
        Tool::Select,
        Tool::Add,
        Tool::Connect,
        Tool::DiagramProps,
        Tool::Clear,
    ];

    /// Whether this entry is a persistent mode (highlighted while active)
    /// vs. a one-shot action button.
    pub fn is_mode(self) -> bool {
        matches!(self, Tool::Select | Tool::Add | Tool::Connect)
    }

    pub fn label(self) -> &'static str {
        match self {
            Tool::Select => "Select",
            Tool::Add => "Add",
            Tool::Connect => "Connect",
            Tool::DiagramProps => "Diagram Properties",
            Tool::Clear => "Clear Selection",
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
    /// A one-shot action button was clicked. The `Tool` payload is kept for the
    /// `log!` in `app.rs` (Debug-only) while these buttons stay mock no-ops.
    Triggered(#[allow(dead_code)] Tool),
}

#[derive(Script, ScriptHook, Widget)]
pub struct ToolDock {
    /// The strip: a `flow: Down` `View` whose `draw_bg` paints the HUD frame and
    /// which lays out the five `IconButton` children.
    #[deref]
    view: View,

    /// Subtle source-bright top edge (shared HUD panel material), drawn over the
    /// frame after the children lay out.
    #[live]
    draw_edge: DrawColor,

    #[rust]
    active: Tool,
}

impl Widget for ToolDock {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        // Drive the children so their `clicked`/hover actions are emitted.
        self.view.handle_event(cx, event, scope);

        let uid = self.widget_uid();
        // Read each child's click (a mode click sets the active mode + emits
        // `ModeChanged`; an action click emits `Triggered`) -- the click->Tool
        // map that replaces the old `item_rects.contains` loop.
        if let Event::Actions(actions) = event {
            for tool in Tool::ALL {
                if self.button(cx, tool).as_icon_button().clicked(actions) {
                    if tool.is_mode() {
                        self.active = tool;
                        self.view.redraw(cx);
                        cx.widget_action(uid, ToolDockAction::ModeChanged(tool));
                    } else {
                        cx.widget_action(uid, ToolDockAction::Triggered(tool));
                    }
                    break;
                }
            }
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        // Sync each child's glyph + lit state before the View lays them out:
        // only the active *mode* button is lit; action buttons never are.
        for tool in Tool::ALL {
            let btn = self.button(cx, tool).as_icon_button();
            btn.set_icon(cx, Self::icon_for(tool));
            btn.set_active(cx, tool.is_mode() && tool == self.active);
        }

        while self.view.draw_walk(cx, scope, walk).step().is_some() {}

        // Reinforce the frame's top border with the source-bright edge line.
        let rect = self.view.area().rect(cx);
        self.draw_edge.draw_abs(
            cx,
            Rect {
                pos: rect.pos,
                size: dvec2(rect.size.x, 1.5),
            },
        );

        DrawStep::done()
    }
}

impl ToolDock {
    /// The child `IconButton` for a tool. Central Tool->widget map, shared by
    /// the draw-time sync and the event-time click read.
    fn button(&mut self, cx: &mut Cx, tool: Tool) -> WidgetRef {
        match tool {
            Tool::Select => self.view.widget(cx, ids!(select_btn)),
            Tool::Add => self.view.widget(cx, ids!(add_btn)),
            Tool::Connect => self.view.widget(cx, ids!(connect_btn)),
            Tool::DiagramProps => self.view.widget(cx, ids!(props_btn)),
            Tool::Clear => self.view.widget(cx, ids!(clear_btn)),
        }
    }

    /// The catalog glyph for a tool. Pure meaning->glyph map; the child
    /// `IconButton` fetches the shader and tints it per-draw.
    fn icon_for(tool: Tool) -> Icon {
        match tool {
            Tool::Select => Icon::MousePointer2,
            Tool::Add => Icon::SquarePlus,
            Tool::Connect => Icon::Spline,
            Tool::DiagramProps => Icon::SlidersHorizontal,
            Tool::Clear => Icon::CircleX,
        }
    }

    /// Set the active mode directly (used by `App` for hotkey-driven
    /// switches, bypassing the click/action round-trip). The next `draw_walk`
    /// re-syncs the child lit states.
    pub fn set_active(&mut self, cx: &mut Cx, tool: Tool) {
        if tool.is_mode() {
            self.active = tool;
            self.view.redraw(cx);
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

#[cfg(test)]
mod icon_map_tests {
    use super::*;
    use crate::icons::Icon;

    #[test]
    fn tool_maps_to_catalog_icon() {
        assert_eq!(ToolDock::icon_for(Tool::Select), Icon::MousePointer2);
        assert_eq!(ToolDock::icon_for(Tool::Add), Icon::SquarePlus);
        assert_eq!(ToolDock::icon_for(Tool::Connect), Icon::Spline);
        assert_eq!(
            ToolDock::icon_for(Tool::DiagramProps),
            Icon::SlidersHorizontal
        );
        assert_eq!(ToolDock::icon_for(Tool::Clear), Icon::CircleX);
    }
}
