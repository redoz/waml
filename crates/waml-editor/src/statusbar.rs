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

#[cfg(test)]
pub fn save_status_line(
    diagram_name: &str,
    node_count: usize,
    zoom_pct: i32,
    tool_label: &str,
    save_error: Option<&str>,
) -> String {
    status_line_with_feedback(
        diagram_name,
        node_count,
        zoom_pct,
        tool_label,
        save_error,
        None,
    )
}

pub fn status_line_with_feedback(
    diagram_name: &str,
    node_count: usize,
    zoom_pct: i32,
    tool_label: &str,
    save_error: Option<&str>,
    navigation_message: Option<&str>,
) -> String {
    prioritized_status_line(
        diagram_name,
        node_count,
        zoom_pct,
        tool_label,
        save_error,
        None,
        None,
        navigation_message,
    )
}

pub fn prioritized_status_line(
    diagram_name: &str,
    node_count: usize,
    zoom_pct: i32,
    tool_label: &str,
    save_error: Option<&str>,
    history_problem: Option<&str>,
    history_success: Option<&str>,
    navigation_message: Option<&str>,
) -> String {
    match save_error {
        Some(error) => format!("Save failed: {error}"),
        None => history_problem
            .or(history_success)
            .or(navigation_message)
            .map(str::to_owned)
            .unwrap_or_else(|| status_line(diagram_name, node_count, zoom_pct, tool_label)),
    }
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
    #[rust]
    save_error: Option<String>,
    #[rust]
    history_problem: Option<String>,
    #[rust]
    history_success: Option<String>,
    #[rust]
    navigation_message: Option<String>,
}

impl Widget for Statusbar {
    fn handle_event(&mut self, _cx: &mut Cx, _event: &Event, _scope: &mut Scope) {
        // Read-only strip -- nothing to hit-test.
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, _scope: &mut Scope, walk: Walk) -> DrawStep {
        let rect = cx.walk_turtle(walk);
        self.draw_bg.draw_abs(cx, rect);
        let line = prioritized_status_line(
            &self.diagram_name,
            self.node_count,
            self.zoom_pct,
            &self.tool_label,
            self.save_error.as_deref(),
            self.history_problem.as_deref(),
            self.history_success.as_deref(),
            self.navigation_message.as_deref(),
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

    pub fn set_save_error(&mut self, cx: &mut Cx, error: Option<&str>) {
        self.save_error = error.map(str::to_owned);
        self.draw_bg.redraw(cx);
    }

    pub fn set_navigation_message(&mut self, cx: &mut Cx, message: Option<&str>) {
        self.navigation_message = message.map(str::to_owned);
        self.draw_bg.redraw(cx);
    }

    pub fn set_history_problem(&mut self, cx: &mut Cx, message: Option<&str>) {
        self.history_problem = message.map(str::to_owned);
        if message.is_some() {
            self.history_success = None;
        }
        self.draw_bg.redraw(cx);
    }

    pub fn set_history_success(&mut self, cx: &mut Cx, message: Option<&str>) {
        self.history_success = message.map(str::to_owned);
        if message.is_some() {
            self.history_problem = None;
        }
        self.draw_bg.redraw(cx);
    }

    pub fn clear_history_feedback(&mut self, cx: &mut Cx) {
        self.history_problem = None;
        self.history_success = None;
        self.draw_bg.redraw(cx);
    }
}

#[cfg(test)]
pub(crate) fn navigation_message(statusbar: &Statusbar) -> Option<&str> {
    statusbar.navigation_message.as_deref()
}

#[cfg(test)]
pub(crate) fn save_error(statusbar: &Statusbar) -> Option<&str> {
    statusbar.save_error.as_deref()
}

#[cfg(test)]
pub(crate) fn history_feedback(statusbar: &Statusbar) -> (Option<&str>, Option<&str>) {
    (
        statusbar.history_problem.as_deref(),
        statusbar.history_success.as_deref(),
    )
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

    #[test]
    fn save_error_replaces_normal_status_with_a_visible_failure() {
        assert_eq!(
            save_status_line("Orders", 3, 100, "Select", Some("disk full")),
            "Save failed: disk full"
        );
    }

    #[test]
    fn navigation_message_replaces_normal_status_with_exact_feedback() {
        let cases = [
            "Invalid link: http://",
            "Unsupported link scheme: mailto",
            "Link leaves this bundle",
            "Document not found: sales/missing",
            "Section not found: missing",
            "Could not open link: blocked",
        ];
        for message in cases {
            assert_eq!(
                status_line_with_feedback("Orders", 3, 100, "Select", None, Some(message),),
                message
            );
        }
    }

    #[test]
    fn save_error_has_priority_over_navigation_feedback() {
        assert_eq!(
            status_line_with_feedback(
                "Orders",
                3,
                100,
                "Select",
                Some("disk full"),
                Some("Section not found: missing"),
            ),
            "Save failed: disk full"
        );
    }

    #[test]
    fn history_feedback_obeys_error_warning_success_navigation_precedence() {
        let line = |save, problem, success, navigation| {
            prioritized_status_line(
                "Orders", 3, 100, "Select", save, problem, success, navigation,
            )
        };
        assert_eq!(
            line(
                Some("disk full"),
                Some("Undo failed"),
                Some("Undid: Rename"),
                Some("Section not found"),
            ),
            "Save failed: disk full"
        );
        assert_eq!(
            line(
                None,
                Some("Undo failed"),
                Some("Undid: Rename"),
                Some("Section not found"),
            ),
            "Undo failed"
        );
        assert_eq!(
            line(None, None, Some("Undid: Rename"), Some("Section not found"),),
            "Undid: Rename"
        );
        assert_eq!(
            line(None, None, None, Some("Section not found")),
            "Section not found"
        );
    }
}
