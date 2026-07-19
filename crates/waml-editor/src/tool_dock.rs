//! Left tool dock (UX mock): a vertical icon strip mirroring the web
//! frontend's toolbox. `Select`/`Add`/`Connect` are the exclusive active
//! tools (mouse click or hotkey V/N/C); `DiagramProps`/`Clear` are one-shot
//! action buttons (no persistent state). Hand-rolled immediate-mode widget,
//! same convention as `doc_tabs.rs`. No tool behavior is wired into the canvas
//! yet -- selecting a tool only changes the dock's own highlight (breadth
//! mock, not polish).
//!
//! Each entry reads like the caption bar's `CaptionButton`: an SDF glyph (the
//! project tree's icon material) in `atlas.text` at rest, tinted `atlas.accent`
//! with a rounded-square accent wash behind it when hovered -- and, for a mode,
//! while it is the active tool.

use makepad_widgets::*;

use crate::icons::TreeIcons;

script_mod! {
    use mod.prelude.widgets_internal.*
    use mod.atlas
    use mod.widgets.*
    use mod.text.*

    // Rounded accent wash behind a hovered/active glyph -- the SAME highlight
    // the caption Save/Menu buttons paint: a premultiplied low-alpha accent
    // square (radius 2), so a dock button reads identically to a caption one.
    // A named DrawColor-with-pixel type (an inline `pixel:` override on a plain
    // field silently draws nothing in this fork; named `mod.draw.*` types render).
    mod.draw.ToolWash = mod.draw.DrawColor{
        pixel: fn() {
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            let a = 0.16
            sdf.box(1.0, 1.0, self.rect_size.x - 2.0, self.rect_size.y - 2.0, 2.0)
            sdf.fill(vec4(self.color.x * a, self.color.y * a, self.color.z * a, a))
            return sdf.result
        }
    }

    mod.widgets.ToolDockBase = #(ToolDock::register_widget(vm))

    mod.widgets.ToolDock = set_type_default() do mod.widgets.ToolDockBase{
        width: 48.0
        height: Fill
        draw_bg: mod.draw.AccentFrame{ color: atlas.field_bg }
        draw_edge +: { color: atlas.frame_hi }
        // Hover/active wash: the caption button's accent highlight (premult
        // accent, faded by `ToolWash`), behind the glyph.
        draw_hover: mod.draw.ToolWash{ color: atlas.accent }
        // Color-only holders: the icon glyphs are DrawColor SDFs whose `color`
        // is set per draw from one of these, so no RGBA crosses Rust.
        draw_icon_lit +: { color: atlas.accent }
        draw_icon_idle +: { color: atlas.text }
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

const ITEM_H: f64 = 44.0;
const ICON_SIZE: f64 = 20.0;
// Side of the rounded accent hover/active wash, centered on the glyph -- the
// caption button's wash side, so the two highlights match.
const WASH_SIZE: f64 = 32.0;
const GROUP_GAP: f64 = 10.0;
// Index of the first action button: the mode group (Select/Add/Connect) ends
// here, so a gap is inserted before it.
const ACTION_START: usize = 3;

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
    /// Rounded accent wash behind a hovered (or active-mode) glyph.
    #[redraw]
    #[live]
    draw_hover: DrawColor,
    /// Color-only holders (never drawn): the icon glyph's `color` is copied from
    /// one of these per draw, so the accent/idle RGBA stays in the DSL.
    #[redraw]
    #[live]
    draw_icon_lit: DrawColor,
    #[redraw]
    #[live]
    draw_icon_idle: DrawColor,
    /// SDF icon set (the project tree's material), drawn per item via
    /// `DrawColor::draw_abs`, tinted per-draw from `draw_icon_lit`/`draw_icon_idle`.
    #[live]
    icons: TreeIcons,

    #[rust]
    active: Tool,
    /// The tool currently under the pointer (caption-button-style hover), or
    /// `None`. Drives the accent glyph + wash.
    #[rust]
    hovered: Option<Tool>,
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
            Hit::FingerHoverIn(fe) | Hit::FingerHoverOver(fe) => {
                cx.set_cursor(MouseCursor::Hand);
                let hit = self
                    .item_rects
                    .iter()
                    .find(|(_, rect)| rect.contains(fe.abs))
                    .map(|(tool, _)| *tool);
                if self.hovered != hit {
                    self.hovered = hit;
                    self.draw_bg.redraw(cx);
                }
            }
            Hit::FingerHoverOut(_) if self.hovered.is_some() => {
                self.hovered = None;
                self.draw_bg.redraw(cx);
            }
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
            if i == ACTION_START {
                y += GROUP_GAP;
            }
            let item_rect = Rect {
                pos: dvec2(rect.pos.x, y),
                size: dvec2(rect.size.x, ITEM_H),
            };
            // "Lit" like a pressed CaptionButton: the pointer is over it, or it
            // is the active mode. Lit => accent glyph + accent wash; else the
            // glyph rests in atlas.text.
            let is_active = tool.is_mode() && tool == self.active;
            let lit = is_active || self.hovered == Some(tool);

            // Every entry reads like a caption Save/Menu button: one centered
            // glyph, no hotkey letter. `+1.0` is the caption glyph's optical
            // down-nudge (a true geometric center sits a hair high).
            let icon_y = item_rect.pos.y + (ITEM_H - ICON_SIZE) * 0.5 + 1.0;
            let cx_mid = rect.pos.x + rect.size.x * 0.5;
            let icon_mid_y = icon_y + ICON_SIZE * 0.5;

            if lit {
                self.draw_hover.draw_abs(
                    cx,
                    Rect {
                        pos: dvec2(
                            (cx_mid - WASH_SIZE * 0.5).round(),
                            (icon_mid_y - WASH_SIZE * 0.5).round(),
                        ),
                        size: dvec2(WASH_SIZE, WASH_SIZE),
                    },
                );
            }

            // No RGBA crosses Rust: the tint is copied from a DSL-declared holder.
            let tint = if lit {
                self.draw_icon_lit.color
            } else {
                self.draw_icon_idle.color
            };
            let icon = Self::icon_for(&mut self.icons, tool);
            icon.color = tint;
            icon.draw_abs(
                cx,
                Rect {
                    pos: dvec2((cx_mid - ICON_SIZE * 0.5).round(), icon_y.round()),
                    size: dvec2(ICON_SIZE, ICON_SIZE),
                },
            );

            self.item_rects.push((tool, item_rect));
            y += ITEM_H;
        }

        DrawStep::done()
    }
}

impl ToolDock {
    /// The SDF icon material for a tool. Takes `&mut TreeIcons` (not `&mut
    /// self`) so the draw loop can borrow the icon without also borrowing the
    /// rest of `self`.
    fn icon_for(icons: &mut TreeIcons, tool: Tool) -> &mut DrawColor {
        match tool {
            Tool::Select => &mut icons.mouse_pointer_2,
            Tool::Add => &mut icons.square_plus,
            Tool::Connect => &mut icons.spline,
            Tool::DiagramProps => &mut icons.sliders_horizontal,
            Tool::Clear => &mut icons.circle_x,
        }
    }

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
