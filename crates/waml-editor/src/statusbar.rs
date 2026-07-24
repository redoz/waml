//! Thin statusbar (UX mock) pinned to the bottom of the whole window (below
//! the tree/canvas/inspector Splitter): current diagram name, node count,
//! zoom %, active tool. Read-only, no interactivity -- just an immediate-mode
//! `DrawText` strip like `doc_tabs.rs`/`tool_dock.rs`, pushed by `App`
//! whenever the active tab, canvas camera, or tool-dock mode changes.
//! Zoom/node-count are snapshot values (pushed on sync points, not live
//! during a canvas drag) -- acceptable for a mock.

use makepad_widgets::*;

script_mod! {
    use mod.prelude.widgets_internal.*
    use mod.atlas
    use mod.widgets.*
    use mod.text.*
    use mod.fonts

    mod.widgets.StatusbarBase = #(Statusbar::register_widget(vm))

    mod.widgets.Statusbar = set_type_default() do mod.widgets.StatusbarBase{
        width: Fill
        height: 24.0
        draw_bg +: { color: atlas.surface }
        draw_text +: {
            color: atlas.text_dim
            text_style: fonts.text_label
        }
    }
}

/// Pure so the join format is unit-tested without a `Cx`.
pub fn status_line(
    diagram_name: &str,
    node_count: usize,
    zoom_pct: i32,
    tool_label: &str,
) -> String {
    let noun = if node_count == 1 { "node" } else { "nodes" };
    format!("{diagram_name}    {node_count} {noun}    Zoom {zoom_pct}%    Tool: {tool_label}")
}

#[derive(Script, ScriptHook, Widget)]
pub struct Statusbar {
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
    draw_text: DrawText,

    #[rust]
    diagram_name: String,
    #[rust]
    node_count: usize,
    #[rust]
    zoom_pct: i32,
    #[rust]
    tool_label: String,
}

impl Widget for Statusbar {
    fn handle_event(&mut self, _cx: &mut Cx, _event: &Event, _scope: &mut Scope) {
        // Read-only strip -- nothing to hit-test.
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, _scope: &mut Scope, walk: Walk) -> DrawStep {
        let rect = cx.walk_turtle(walk);
        self.draw_bg.draw_abs(cx, rect);
        let line = status_line(
            &self.diagram_name,
            self.node_count,
            self.zoom_pct,
            &self.tool_label,
        );
        let text_y = rect.pos.y + rect.size.y * 0.5 - 6.0;
        self.draw_text
            .draw_abs(cx, dvec2(rect.pos.x + 12.0, text_y), &line);
        DrawStep::done()
    }
}

impl Statusbar {
    pub fn set_state(
        &mut self,
        cx: &mut Cx,
        diagram_name: String,
        node_count: usize,
        zoom_pct: i32,
        tool_label: &str,
    ) {
        self.diagram_name = diagram_name;
        self.node_count = node_count;
        self.zoom_pct = zoom_pct;
        self.tool_label = tool_label.to_string();
        self.draw_bg.redraw(cx);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn joins_all_four_fields() {
        assert_eq!(
            status_line("Orders", 3, 100, "Select"),
            "Orders    3 nodes    Zoom 100%    Tool: Select"
        );
    }

    #[test]
    fn singular_node_noun_for_one() {
        assert_eq!(
            status_line("Orders", 1, 150, "Add"),
            "Orders    1 node    Zoom 150%    Tool: Add"
        );
    }
}
