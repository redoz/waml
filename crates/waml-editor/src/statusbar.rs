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
        // Per-agent window marker (`--title` / `--color`), right-aligned in this
        // strip. It belongs in the caption's title row and is mounted there too,
        // but that row cannot show it: every seat in it is painted over by a
        // `Size::Fill` sibling, which makepad defers past anything declared
        // after it. The statusbar has no such sibling and is the last thing the
        // body draws, so the mark is parked here until the caption's marker gets
        // a pass of its own.
        chip_fallback: atlas.selection
        ink_fallback: atlas.text
        draw_chip +: {
            color: #0000
            pixel: fn() {
                let sdf = Sdf2d.viewport(self.pos * self.rect_size)
                sdf.box(0.0, 0.0, self.rect_size.x, self.rect_size.y, 2.5)
                sdf.fill(self.color)
                return sdf.result
            }
        }
    }
}

/// Marker pill padding, and its gap from the strip's right edge.
const CHIP_PAD_X: f64 = 6.0;
const CHIP_PAD_Y: f64 = 2.0;
const CHIP_RIGHT_GAP: f64 = 10.0;

/// Inner width of a `--color`-only pill, which carries no text to size it.
const SWATCH_W: f64 = 14.0;

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

#[cfg(test)]
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
        StatusFeedback {
            save_error,
            navigation_message,
            ..Default::default()
        },
    )
}

#[derive(Clone, Copy, Debug, Default)]
pub struct StatusFeedback<'a> {
    pub save_error: Option<&'a str>,
    pub history_problem: Option<&'a str>,
    pub history_success: Option<&'a str>,
    pub navigation_message: Option<&'a str>,
    /// The active document's solver diagnostics (behavior canvases, spec §5.3).
    /// Lowest priority: it is standing state, not feedback about a just-taken
    /// action, so any of the above replaces it.
    pub solver_diagnostics: Option<&'a str>,
}

pub fn prioritized_status_line(
    diagram_name: &str,
    node_count: usize,
    zoom_pct: i32,
    tool_label: &str,
    feedback: StatusFeedback<'_>,
) -> String {
    match feedback.save_error {
        Some(error) => format!("Save failed: {error}"),
        None => feedback
            .history_problem
            .or(feedback.history_success)
            .or(feedback.navigation_message)
            .or(feedback.solver_diagnostics)
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
    #[rust]
    solver_diagnostics: Option<String>,

    #[redraw]
    #[live]
    draw_chip: DrawColor,
    #[live]
    chip_fallback: Vec4,
    #[live]
    ink_fallback: Vec4,
    /// Badge text from `--title`; `None` draws no pill.
    #[rust]
    agent_badge: Option<String>,
    /// Tint from `--color`; `None` falls back to `chip_fallback`.
    #[rust]
    agent_tint: Option<Vec4>,
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
            StatusFeedback {
                save_error: self.save_error.as_deref(),
                history_problem: self.history_problem.as_deref(),
                history_success: self.history_success.as_deref(),
                navigation_message: self.navigation_message.as_deref(),
                solver_diagnostics: self.solver_diagnostics.as_deref(),
            },
        );
        let text_y = rect.pos.y + rect.size.y * 0.5 - 6.0;
        self.draw_text
            .draw_abs(cx, dvec2(rect.pos.x + 12.0, text_y), &line);
        self.draw_agent_mark(cx, rect, text_y);
        DrawStep::done()
    }
}

impl Statusbar {
    /// Push the parsed `--title`/`--color` values in; called at startup and
    /// again after a theme reload (which wipes `#[rust]` fields).
    pub fn set_agent_marks(&mut self, cx: &mut Cx, badge: Option<String>, tint: Option<Vec4>) {
        self.agent_badge = badge;
        self.agent_tint = tint;
        self.redraw(cx);
    }

    /// The marker pill, right-aligned in `rect`. Shares the status line's
    /// baseline rather than centring on its own measured band: the strip is
    /// 24px and the pill is only ever beside that one line, so seating them
    /// together is what keeps the row from looking double-decked.
    fn draw_agent_mark(&mut self, cx: &mut Cx2d, rect: Rect, text_y: f64) {
        if self.agent_badge.is_none() && self.agent_tint.is_none() {
            return;
        }
        let text = self.agent_badge.clone().unwrap_or_default();
        let inner_w = if text.is_empty() {
            SWATCH_W
        } else {
            self.draw_text
                .layout(cx, 0.0, 0.0, None, false, Align::default(), &text)
                .size_in_lpxs
                .width as f64
        };
        let line_h = self
            .draw_text
            .layout(cx, 0.0, 0.0, None, false, Align::default(), "Hg")
            .size_in_lpxs
            .height as f64;

        let chip = Rect {
            pos: dvec2(
                (rect.pos.x + rect.size.x - inner_w - CHIP_PAD_X * 2.0 - CHIP_RIGHT_GAP).round(),
                (text_y - CHIP_PAD_Y).round(),
            ),
            size: dvec2(inner_w + CHIP_PAD_X * 2.0, line_h + CHIP_PAD_Y * 2.0),
        };

        let fill = self.agent_tint.unwrap_or(self.chip_fallback);
        self.draw_chip.color = fill;
        self.draw_chip.draw_abs(cx, chip);

        if !text.is_empty() {
            // Ink is picked against the FILL, not the strip: a `--color` pill is
            // an arbitrary hue and the status ink would vanish on half of them.
            //
            // Set-and-restore around the draw. The status line shares this
            // handle and its colour is the DSL default (`atlas.text_dim`), NOT
            // `ink_fallback` -- restoring to the wrong one silently repaints the
            // whole line a shade darker from the next frame on.
            let line_ink = self.draw_text.color;
            self.draw_text.color = if self.agent_tint.is_some() {
                crate::agent_mark::label_ink(fill)
            } else {
                self.ink_fallback
            };
            self.draw_text
                .draw_abs(cx, dvec2(chip.pos.x + CHIP_PAD_X, text_y), &text);
            self.draw_text.color = line_ink;
        }
    }

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

    /// Push the active document's solver diagnostics (spec §5.3).
    pub fn set_solver_diagnostics(&mut self, cx: &mut Cx, message: Option<&str>) {
        self.solver_diagnostics = message.map(str::to_owned);
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
pub(crate) fn solver_diagnostics(statusbar: &Statusbar) -> Option<&str> {
    statusbar.solver_diagnostics.as_deref()
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

    /// Solver diagnostics are the lowest-priority line: they show instead of
    /// the plain status line, and any action feedback replaces them.
    #[test]
    fn solver_diagnostics_sit_below_every_action_feedback() {
        let line = |navigation, diagnostics| {
            prioritized_status_line(
                "Orders",
                3,
                100,
                "Select",
                StatusFeedback {
                    navigation_message: navigation,
                    solver_diagnostics: diagnostics,
                    ..Default::default()
                },
            )
        };
        assert_eq!(
            line(None, Some("2 diagnostics: unreachable")),
            "2 diagnostics: unreachable"
        );
        assert_eq!(
            line(
                Some("Section not found"),
                Some("2 diagnostics: unreachable")
            ),
            "Section not found"
        );
        assert_eq!(line(None, None), status_line("Orders", 3, 100, "Select"));
    }

    #[test]
    fn history_feedback_obeys_error_warning_success_navigation_precedence() {
        let line = |save, problem, success, navigation| {
            prioritized_status_line(
                "Orders",
                3,
                100,
                "Select",
                StatusFeedback {
                    save_error: save,
                    history_problem: problem,
                    history_success: success,
                    navigation_message: navigation,
                    solver_diagnostics: None,
                },
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
