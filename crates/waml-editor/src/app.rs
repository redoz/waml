use crate::doc_tabs::{OpenTabs, TabKind};
use crate::inspector::{diagram_elements, Subject};
use crate::load;
use crate::scene::{build_focus_scene, build_scene};
use makepad_widgets::*;
use std::path::Path;
use waml::model::Model;

script_mod! {
    use mod.prelude.widgets.*
    use mod.atlas
    use mod.widgets.GraphCanvas
    use mod.widgets.ProjectTree
    use mod.widgets.Inspector
    use mod.widgets.DocTabs
    use mod.widgets.DiagramSwitcher
    use mod.widgets.ShortcutsOverlay
    use mod.widgets.ToolDock
    use mod.widgets.SelectionToolbar
    use mod.widgets.Statusbar
    use mod.widgets.SolidView
    use mod.widgets.DesktopButton
    use mod.widgets.DesktopButtonType
    use mod.widgets.StartScreen

    startup() do #(App::script_component(vm)){
        ui: Root{
            main_window := Window{
                window.inner_size: vec2(1280, 840)
                window.title: "WAML"
                window.caption_bar_height_override: 44.0
                caption_bar: SolidView{
                    visible: false
                    flow: Right
                    height: Fit
                    draw_bg.color: atlas.field_bg
                    // Fixed-width nav cluster so the first doc tab's left edge
                    // lines up with the project tree's right edge (12 margin +
                    // 280 tree width = 292).
                    nav := View{
                        width: 292.0
                        height: Fill
                        flow: Right
                        align: Align{y: 0.5}
                    wordmark := View{
                        width: Fit
                        height: Fill
                        align: Align{x: 0.0, y: 0.5}
                        margin: Inset{left: 2.0}
                        padding: Inset{left: 6.0, right: 10.0}
                        // 6-color "W" wordmark, drawn as an anti-aliased SDF (see
                        // `logo.rs`) -- DrawSvg stair-stepped at this size. Fixed
                        // size holds the logo's ~1.749 content aspect. `SolidView`
                        // (not `View`) because its `draw_bg` is a `DrawColor`, so
                        // the `LogoMark` DrawColor subclass can attach; a plain
                        // `View`'s `draw_bg` is a `DrawQuad` and rejects it.
                        SolidView{
                            width: 45.5
                            height: 26.0
                            draw_bg: mod.draw.LogoMark{}
                        }
                    }
                    sep := View{
                        width: Fit
                        height: Fill
                        align: Align{x: 0.0, y: 0.5}
                        Label{
                            text: "/"
                            draw_text +: {
                                color: atlas.text_dim
                                text_style: theme.font_regular{font_size: 16}
                            }
                        }
                    }
                    pkg_name_view := View{
                        width: Fit
                        height: Fill
                        align: Align{x: 0.0, y: 0.5}
                        pkg_name := Label{
                            text: ""
                            draw_text +: {
                                color: atlas.text_dim
                                // Inline font with the same asc/desc trim as the
                                // doc tabs so the metric box centers on the glyphs
                                // (theme.font_regular rides high when box-centered).
                                text_style: TextStyle{
                                    font_size: 13
                                    font_family: FontFamily{
                                        latin := FontMember{res: crate_resource("self:resources/fonts/IBM_Plex_Sans/IBMPlexSans-Regular.ttf") asc: -0.1 desc: 0.0}
                                    }
                                }
                            }
                        }
                    }
                    }
                    doc_tabs := DocTabs{
                        width: Fill
                        height: Fill
                    }
                    windows_buttons := View {
                        visible: false
                        width: Fit height: Fit
                        min := DesktopButton {
                            draw_bg.button_type: DesktopButtonType.WindowsMin
                            width: 46 height: 29
                            draw_bg +: {
                                color: #000, color_hover: #000, color_down: #000
                                bg_color_hover: #E9E9E9, bg_color_down: #CCCCCC
                            }
                        }
                        max := DesktopButton {
                            draw_bg.button_type: DesktopButtonType.WindowsMax
                            width: 46 height: 29
                            draw_bg +: {
                                color: #000, color_hover: #000, color_down: #000
                                bg_color_hover: #E9E9E9, bg_color_down: #CCCCCC
                            }
                        }
                        close := DesktopButton {
                            draw_bg.button_type: DesktopButtonType.WindowsClose
                            width: 46 height: 29
                            draw_bg +: {
                                color: #000, color_hover: #FFF, color_down: #FFF
                                bg_color_hover: #E81123, bg_color_down: #F1707A
                            }
                        }
                    }
                }
                body +: {
                    View{
                    width: Fill
                    height: Fill
                    // Overlay flow: `main_column` and `shortcuts_overlay`
                    // both get the full turtle rect (see `Flow::Overlay` in
                    // makepad's turtle.rs); `shortcuts_overlay` is declared
                    // second, so it paints over the whole column when open,
                    // and draws nothing when closed. Plain flow:Down
                    // siblings can't do this -- Fill/Fill would split space
                    // between them instead of overlapping (see U7's paint-
                    // order writeup on `DiagramSwitcher` for the sibling
                    // z-order rules this sidesteps).
                    flow: Overlay
                    main_column := View{
                    width: Fill
                    height: Fill
                    flow: Down
                    // Starts hidden: the start screen (no-arg launch) shows
                    // over this; `App` flips it visible once a project opens.
                    visible: false
                    // Body: a fullscreen canvas base with floating HUD panels
                    // over it. In an Overlay flow every child gets the full body
                    // rect, so each panel is wrapped in a Fill/Fill View whose
                    // `align` parks it in a corner/edge; the panel's own margin
                    // leaves canvas ground showing around it. The wrappers carry
                    // no bg and don't grab pointer events over empty area, so the
                    // canvas keeps its pan/zoom in the gaps between panels.
                    View{
                        width: Fill
                        height: Fill
                        flow: Overlay
                        canvas := GraphCanvas{
                            width: Fill
                            height: Fill
                        }
                        // Tool dock: left edge, vertically centered.
                        View{
                            width: Fill
                            height: Fill
                            align: Align{x: 0.0, y: 0.5}
                            tool_dock := ToolDock{
                                width: 48.0
                                // Hugs its 7 buttons (7 * ITEM_H 44); the widget
                                // draws items manually so Fit collapses to 0 --
                                // an explicit height is required. Vertically
                                // centered, right of the project tree (12+280+12).
                                height: 308.0
                                margin: Inset{left: 304.0}
                            }
                        }
                        // Project tree: far left edge.
                        View{
                            width: Fill
                            height: Fill
                            align: Align{x: 0.0, y: 0.0}
                            project_tree := ProjectTree{
                                width: 280.0
                                height: Fill
                                margin: Inset{left: 12.0, top: 12.0, bottom: 12.0}
                            }
                        }
                        // Inspector: top-right.
                        View{
                            width: Fill
                            height: Fill
                            align: Align{x: 1.0, y: 0.0}
                            inspector := Inspector{
                                width: 320.0
                                height: Fill
                                margin: Inset{right: 12.0, top: 12.0, bottom: 12.0}
                            }
                        }
                        // Selection toolbar: bottom, centered.
                        View{
                            width: Fill
                            height: Fill
                            align: Align{x: 0.5, y: 1.0}
                            selection_toolbar := SelectionToolbar{
                                width: Fit
                                height: 44.0
                                margin: Inset{bottom: 12.0}
                            }
                        }
                    }
                    }
                    shortcuts_overlay := ShortcutsOverlay{
                        width: Fill
                        height: Fill
                    }
                    start_screen := StartScreen{
                        width: Fill
                        height: Fill
                    }
                    }
                }
            }
        }
    }
}

#[derive(Script, ScriptHook)]
pub struct App {
    #[live]
    ui: WidgetRef,
    #[rust]
    model: Model,
    #[rust]
    tabs: OpenTabs,
    /// Recents last rendered into the start screen, so an `OpenRecent(i)`
    /// action resolves to a path without re-reading disk or index drift.
    #[rust]
    start_recents: Vec<crate::config::Recent>,
}

impl App {
    /// Point the canvas + inspector at the currently active doc tab. Diagram
    /// tabs rebuild+fit the full diagram scene (inspector empty state, since
    /// diagram hit-test selection is out of scope); classifier tabs pin the
    /// 1.5x focus render and point the inspector at that classifier.
    fn sync_active_tab(&mut self, cx: &mut Cx) {
        let Some(active) = self.tabs.active_tab().cloned() else {
            return;
        };
        match active.kind {
            TabKind::Diagram => {
                let built = self
                    .model
                    .diagrams
                    .iter()
                    .find(|d| d.key == active.key)
                    .map(|d| build_scene(&self.model, d));
                if let Some((scene, diags)) = built {
                    for d in &diags {
                        log!("diagnostic: {d:?}");
                    }
                    let node_keys: Vec<String> =
                        scene.nodes.iter().map(|n| n.key.clone()).collect();
                    if let Some(mut canvas) = self
                        .ui
                        .widget(cx, ids!(canvas))
                        .borrow_mut::<crate::canvas::GraphCanvas>()
                    {
                        canvas.set_scene(cx, scene);
                    }
                    self.sync_inspector_elements(cx, &active.key, &active.title, &node_keys);
                }
                if let Some(mut inspector) = self
                    .ui
                    .widget(cx, ids!(inspector))
                    .borrow_mut::<crate::inspector_panel::Inspector>()
                {
                    inspector.set_subject(cx, &self.model, Subject::None);
                }
                if let Some(mut toolbar) =
                    self.ui
                        .widget(cx, ids!(selection_toolbar))
                        .borrow_mut::<crate::selection_toolbar::SelectionToolbar>()
                {
                    toolbar.set_selection(cx, None);
                }
            }
            TabKind::Classifier => {
                let scene = build_focus_scene(&self.model, &active.key);
                if let Some(mut canvas) = self
                    .ui
                    .widget(cx, ids!(canvas))
                    .borrow_mut::<crate::canvas::GraphCanvas>()
                {
                    canvas.set_focus(cx, scene);
                }
                if let Some(mut inspector) = self
                    .ui
                    .widget(cx, ids!(inspector))
                    .borrow_mut::<crate::inspector_panel::Inspector>()
                {
                    inspector.set_subject(cx, &self.model, Subject::Classifier(active.key.clone()));
                }
                if let Some(mut toolbar) =
                    self.ui
                        .widget(cx, ids!(selection_toolbar))
                        .borrow_mut::<crate::selection_toolbar::SelectionToolbar>()
                {
                    // Single-classifier focus only in this mock -- always 1.
                    toolbar.set_selection(cx, Some(1));
                }
            }
        }
        self.sync_statusbar(cx);
    }

    fn refresh_doc_tabs(&mut self, cx: &mut Cx) {
        if let Some(mut doc_tabs) = self
            .ui
            .widget(cx, ids!(doc_tabs))
            .borrow_mut::<crate::doc_tabs::DocTabs>()
        {
            doc_tabs.set_tabs(cx, &self.tabs);
        }
    }

    /// Swap the base (first) tab's diagram and re-activate it. Shared by the
    /// tree panel's diagram row and the caption-area diagram switcher (U7) --
    /// both just need to name a diagram key, everything else is identical.
    fn switch_diagram(&mut self, cx: &mut Cx, key: &str) {
        let Some(diagram) = self.model.diagrams.iter().find(|d| d.key == key) else {
            log!("switch_diagram: no diagram with key {key:?}");
            return;
        };
        let base_id = self
            .tabs
            .set_diagram_base(diagram.key.clone(), diagram.title.clone());
        self.tabs.activate(base_id);
        self.refresh_doc_tabs(cx);
        self.sync_active_tab(cx);
        self.sync_diagram_switcher_current(cx);
    }

    /// Push the base tab's current diagram title into the switcher's trigger
    /// chip. Called wherever the base tab's diagram changes.
    fn sync_diagram_switcher_current(&mut self, cx: &mut Cx) {
        let title = self
            .tabs
            .tabs
            .iter()
            .find(|t| t.kind == TabKind::Diagram)
            .map(|t| t.title.clone())
            .unwrap_or_default();
        if let Some(mut switcher) = self
            .ui
            .widget(cx, ids!(diagram_switcher))
            .borrow_mut::<crate::diagram_switcher::DiagramSwitcher>()
        {
            switcher.set_current(cx, &title);
        }
    }

    /// Feed the inspector's element-picker the current diagram's contents
    /// (diagram, nodes, source-anchored edges). `node_keys` are the diagram's
    /// drawable nodes, in display order (from the built `Scene`).
    fn sync_inspector_elements(
        &mut self,
        cx: &mut Cx,
        diagram_key: &str,
        diagram_title: &str,
        node_keys: &[String],
    ) {
        let rows = diagram_elements(&self.model, diagram_key, diagram_title, node_keys);
        if let Some(mut inspector) = self
            .ui
            .widget(cx, ids!(inspector))
            .borrow_mut::<crate::inspector_panel::Inspector>()
        {
            inspector.set_diagram_elements(cx, rows);
        }
    }

    /// Toggle the keybinding-hint overlay (U8), triggered by the tool
    /// dock's `Shortcuts` button or the `?` hotkey.
    fn toggle_shortcuts_overlay(&mut self, cx: &mut Cx) {
        if let Some(mut overlay) = self
            .ui
            .widget(cx, ids!(shortcuts_overlay))
            .borrow_mut::<crate::shortcuts_overlay::ShortcutsOverlay>()
        {
            let next = !overlay.visible();
            overlay.set_visible(cx, next);
        }
    }

    /// Force the overlay's visibility (used by the `Escape` hotkey, which
    /// should only ever close it, never toggle it open).
    fn set_shortcuts_overlay(&mut self, cx: &mut Cx, visible: bool) {
        if let Some(mut overlay) = self
            .ui
            .widget(cx, ids!(shortcuts_overlay))
            .borrow_mut::<crate::shortcuts_overlay::ShortcutsOverlay>()
        {
            overlay.set_visible(cx, visible);
        }
    }

    /// Push diagram name / node count / zoom / active tool into the bottom
    /// statusbar. Snapshot values -- called at each sync point (tab switch,
    /// startup, tool-dock mode change), not live during a canvas drag.
    fn sync_statusbar(&mut self, cx: &mut Cx) {
        let diagram_name = self
            .tabs
            .tabs
            .first()
            .map(|t| t.title.clone())
            .unwrap_or_default();
        let (node_count, zoom_pct) = self
            .ui
            .widget(cx, ids!(canvas))
            .borrow_mut::<crate::canvas::GraphCanvas>()
            .map(|c| (c.node_count(), c.zoom_pct()))
            .unwrap_or((0, 100));
        let tool_label = self
            .ui
            .widget(cx, ids!(tool_dock))
            .borrow_mut::<crate::tool_dock::ToolDock>()
            .map(|d| d.active().label())
            .unwrap_or("Select");
        if let Some(mut statusbar) = self
            .ui
            .widget(cx, ids!(statusbar))
            .borrow_mut::<crate::statusbar::Statusbar>()
        {
            statusbar.set_state(cx, diagram_name, node_count, zoom_pct, tool_label);
        }
    }

    /// Load `dir` and populate the editor (tree, canvas, tabs, inspector,
    /// statusbar, diagram switcher). Returns `false` (having `log!`d) if the
    /// model fails to load or has no diagrams -- the caller then leaves the
    /// start screen up rather than revealing a blank editor.
    fn open_dir(&mut self, cx: &mut Cx, dir: &Path, wanted_diagram: Option<&str>) -> bool {
        let model = match load::load_model(dir) {
            Ok(m) => m,
            Err(e) => {
                log!("failed to load OKF dir {:?}: {e}", dir);
                return false;
            }
        };
        self.model = model;

        let root_name = if self.model.path.is_empty() {
            "bundle"
        } else {
            self.model.path.as_str()
        };
        self.ui.label(cx, ids!(pkg_name)).set_text(cx, root_name);

        // Record this open in the recents store (best-effort; see config.rs).
        crate::config::push_recent(dir, root_name);

        let tree = crate::tree::build_tree(&self.model);
        if let Some(mut panel) = self
            .ui
            .widget(cx, ids!(project_tree))
            .borrow_mut::<crate::tree_panel::ProjectTree>()
        {
            panel.set_tree(cx, tree);
        } else {
            log!("project_tree widget not found / wrong type");
        }

        let Some(diagram) = crate::cli::select_diagram(&self.model, wanted_diagram) else {
            log!("no diagrams in {:?}", dir);
            return false;
        };
        let (scene, diags) = build_scene(&self.model, diagram);
        for d in &diags {
            log!("diagnostic: {d:?}");
        }
        let diagram_key = diagram.key.clone();
        let diagram_title = diagram.title.clone();
        let node_keys: Vec<String> = scene.nodes.iter().map(|n| n.key.clone()).collect();
        if let Some(mut canvas) = self
            .ui
            .widget(cx, ids!(canvas))
            .borrow_mut::<crate::canvas::GraphCanvas>()
        {
            canvas.set_scene(cx, scene);
        } else {
            log!("canvas widget not found / wrong type");
        }

        self.tabs = OpenTabs::diagram_base(diagram.key.clone(), diagram.title.clone());
        self.refresh_doc_tabs(cx);
        if let Some(mut inspector) = self
            .ui
            .widget(cx, ids!(inspector))
            .borrow_mut::<crate::inspector_panel::Inspector>()
        {
            inspector.set_subject(cx, &self.model, Subject::None);
        }
        self.sync_inspector_elements(cx, &diagram_key, &diagram_title, &node_keys);
        self.sync_statusbar(cx);

        // Diagram switcher (U7): push the base tab's current diagram title
        // into the trigger chip once here.
        self.sync_diagram_switcher_current(cx);
        true
    }

    /// Reveal the editor, hide the start screen. `main_column` is a `View`
    /// (honors `WidgetRef::set_visible`); `StartScreen` is a custom widget
    /// whose no-op default `Widget::set_visible` means we must toggle its own
    /// `visible` flag via the borrowed inherent method instead.
    fn show_editor(&mut self, cx: &mut Cx) {
        self.ui.widget(cx, ids!(main_column)).set_visible(cx, true);
        if let Some(mut screen) = self
            .ui
            .widget(cx, ids!(start_screen))
            .borrow_mut::<crate::start_screen::StartScreen>()
        {
            screen.set_visible(cx, false);
        }
    }

    /// Load recents into the start screen and reveal it, hiding the editor.
    fn show_start_screen(&mut self, cx: &mut Cx) {
        self.start_recents = crate::config::recents();
        let rows: Vec<crate::start_screen::RecentRow> = self
            .start_recents
            .iter()
            .map(|r| crate::start_screen::RecentRow {
                title: r.title().to_string(),
                path: r.path().display().to_string(),
                when: format_opened(r.opened_at()),
            })
            .collect();
        if let Some(mut screen) = self
            .ui
            .widget(cx, ids!(start_screen))
            .borrow_mut::<crate::start_screen::StartScreen>()
        {
            screen.set_recents(cx, rows);
            screen.set_visible(cx, true);
        }
        self.ui.widget(cx, ids!(main_column)).set_visible(cx, false);
    }
}

/// Humanize a recent's `opened_at` (unix seconds) as a coarse relative stamp
/// ("just now", "yesterday", "3 weeks ago") for the start-screen row -- easier
/// to scan than an absolute date and self-explanatory without a header.
fn format_opened(secs: u64) -> String {
    const MIN: u64 = 60;
    const HOUR: u64 = 60 * MIN;
    const DAY: u64 = 24 * HOUR;
    const WEEK: u64 = 7 * DAY;
    const MONTH: u64 = 30 * DAY;
    const YEAR: u64 = 365 * DAY;

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(secs);
    let d = now.saturating_sub(secs);

    // "1 unit ago" reads better as "a unit ago"; "an hour" is special-cased.
    fn ago(n: u64, unit: &str) -> String {
        if n == 1 {
            format!("a {unit} ago")
        } else {
            format!("{n} {unit}s ago")
        }
    }
    match d {
        0..=44 => "just now".to_string(),
        45..=89 => "a minute ago".to_string(),
        _ if d < HOUR => ago(d / MIN, "minute"),
        _ if d < 2 * HOUR => "an hour ago".to_string(),
        _ if d < DAY => ago(d / HOUR, "hour"),
        _ if d < 2 * DAY => "yesterday".to_string(),
        _ if d < WEEK => ago(d / DAY, "day"),
        _ if d < 2 * WEEK => "a week ago".to_string(),
        _ if d < MONTH => ago(d / WEEK, "week"),
        _ if d < 2 * MONTH => "a month ago".to_string(),
        _ if d < YEAR => ago(d / MONTH, "month"),
        _ if d < 2 * YEAR => "a year ago".to_string(),
        _ => ago(d / YEAR, "year"),
    }
}

impl MatchEvent for App {
    fn handle_startup(&mut self, cx: &mut Cx) {
        let argv: Vec<String> = std::env::args().collect();
        let args = match crate::cli::parse(&argv) {
            Ok(a) => a,
            Err(e) => {
                log!("{e}");
                return;
            }
        };
        match args.dir {
            Some(dir) => {
                if self.open_dir(cx, &dir, args.diagram.as_deref()) {
                    self.show_editor(cx);
                } else {
                    // Bad dir -> fall back to the start screen, never a blank window.
                    self.show_start_screen(cx);
                }
            }
            None => self.show_start_screen(cx),
        }
    }

    fn handle_actions(&mut self, cx: &mut Cx, actions: &Actions) {
        // Classifier focus: single-click a class row -> open/replace the
        // single preview tab, focus-render that node in the canvas, and
        // point the inspector at it.
        let focused = self
            .ui
            .widget(cx, ids!(project_tree))
            .borrow_mut::<crate::tree_panel::ProjectTree>()
            .and_then(|panel| panel.focused_classifier(actions));
        if let Some(key) = focused {
            if let Some(node) = self.model.nodes.iter().find(|n| n.key == key) {
                let title = node
                    .concept
                    .title
                    .clone()
                    .unwrap_or_else(|| node.key.clone());
                self.tabs
                    .open_preview(key, title, crate::tree::kind_of(&node.ty));
                self.refresh_doc_tabs(cx);
                self.sync_active_tab(cx);
            }
            return;
        }

        // Diagram row: swap the permanent Diagram tab's content and activate it.
        let selected = self
            .ui
            .widget(cx, ids!(project_tree))
            .borrow_mut::<crate::tree_panel::ProjectTree>()
            .and_then(|panel| panel.selected_diagram(actions));
        if let Some(key) = selected {
            self.switch_diagram(cx, &key);
            return;
        }

        // Diagram switcher chip (caption area, U7): click cycles the base
        // tab to the next `Model::diagrams` entry (wrapping), same
        // swap-and-activate path as the tree panel's diagram row.
        let switcher_clicked = self
            .ui
            .widget(cx, ids!(diagram_switcher))
            .borrow_mut::<crate::diagram_switcher::DiagramSwitcher>()
            .and_then(|switcher| switcher.switcher_action(actions));
        if let Some(crate::diagram_switcher::DiagramSwitcherAction::Clicked) = switcher_clicked {
            let keys: Vec<String> = self.model.diagrams.iter().map(|d| d.key.clone()).collect();
            let current = self
                .tabs
                .tabs
                .iter()
                .find(|t| t.kind == TabKind::Diagram)
                .map(|t| t.key.clone())
                .unwrap_or_default();
            if let Some(next) = crate::diagram_switcher::next_diagram_key(&keys, &current) {
                self.switch_diagram(cx, &next);
            }
            return;
        }

        // Inline-edit commit: the inspector emits `Edited(subject_key)` when a
        // field's value actually changed. Promote the tab pointing at that
        // subject from preview to persisted (title de-dims).
        let edited_key = self
            .ui
            .widget(cx, ids!(inspector))
            .borrow_mut::<crate::inspector_panel::Inspector>()
            .and_then(|inspector| inspector.edited(actions));
        if let Some(key) = edited_key {
            if let Some(tab) = self.tabs.tabs.iter().find(|t| t.key == key) {
                let id = tab.id;
                self.tabs.promote(id);
                self.refresh_doc_tabs(cx);
            }
            return;
        }

        // Element-picker dropdown: a node pick repoints the inspector only
        // (inspector-local -- no tab open, no canvas move), the same path a
        // canvas/tab selection takes.
        let picked = self
            .ui
            .widget(cx, ids!(inspector))
            .borrow_mut::<crate::inspector_panel::Inspector>()
            .and_then(|inspector| inspector.picked(actions));
        if let Some(key) = picked {
            if let Some(mut inspector) = self
                .ui
                .widget(cx, ids!(inspector))
                .borrow_mut::<crate::inspector_panel::Inspector>()
            {
                inspector.set_subject(cx, &self.model, Subject::Classifier(key));
            }
            return;
        }

        // Tool dock: mode clicks (Select/Add/Connect) update their own
        // highlight already; `Shortcuts` toggles the keybinding overlay
        // (U8); the rest of the one-shot action buttons stay mock no-ops.
        let dock_action = self
            .ui
            .widget(cx, ids!(tool_dock))
            .borrow_mut::<crate::tool_dock::ToolDock>()
            .and_then(|dock| dock.dock_action(actions));
        if let Some(action) = dock_action {
            match action {
                crate::tool_dock::ToolDockAction::ModeChanged(_) => self.sync_statusbar(cx),
                crate::tool_dock::ToolDockAction::Triggered(crate::tool_dock::Tool::Shortcuts) => {
                    self.toggle_shortcuts_overlay(cx);
                }
                other => log!("tool dock: {other:?}"),
            }
            return;
        }

        // Start screen: recent-project rows open directly; New/Open project
        // stay stubs (`log!` only) until the template picker / rfd dialog
        // land in a later slice.
        if let Some(screen) = self
            .ui
            .widget(cx, ids!(start_screen))
            .borrow_mut::<crate::start_screen::StartScreen>()
        {
            if let Some(action) = screen.screen_action(actions) {
                drop(screen); // release the borrow before opening a project
                match action {
                    crate::start_screen::StartScreenAction::OpenRecent(i) => {
                        if let Some(recent) = self.start_recents.get(i).cloned() {
                            if self.open_dir(cx, recent.path(), None) {
                                self.show_editor(cx);
                            }
                        }
                    }
                    crate::start_screen::StartScreenAction::NewProject => {
                        log!("New project: not yet implemented (template picker is a later slice)");
                    }
                    crate::start_screen::StartScreenAction::OpenProject => {
                        log!("Open project: not yet implemented (rfd picker is a later slice)");
                    }
                    crate::start_screen::StartScreenAction::None => {}
                }
                return;
            }
        }

        // Shortcuts overlay (U8): `?` (dock button or hotkey) or clicking
        // anywhere on the overlay's scrim closes it again.
        let overlay_dismissed = self
            .ui
            .widget(cx, ids!(shortcuts_overlay))
            .borrow_mut::<crate::shortcuts_overlay::ShortcutsOverlay>()
            .and_then(|overlay| overlay.overlay_action(actions));
        if let Some(crate::shortcuts_overlay::ShortcutsOverlayAction::Dismissed) = overlay_dismissed
        {
            self.toggle_shortcuts_overlay(cx);
            return;
        }

        // Selection toolbar: `Delete` closes the focused classifier's doc
        // tab (in-memory only -- the Model is never touched); `New Diagram`
        // is a mock no-op (diagram creation is out of scope for this pass).
        let toolbar_action = self
            .ui
            .widget(cx, ids!(selection_toolbar))
            .borrow_mut::<crate::selection_toolbar::SelectionToolbar>()
            .and_then(|toolbar| toolbar.toolbar_action(actions));
        match toolbar_action {
            Some(crate::selection_toolbar::SelectionToolbarAction::Delete) => {
                if let Some(active) = self.tabs.active_tab() {
                    if active.kind == TabKind::Classifier {
                        let id = active.id;
                        self.tabs.close(id);
                        self.refresh_doc_tabs(cx);
                        self.sync_active_tab(cx);
                    }
                }
                return;
            }
            Some(crate::selection_toolbar::SelectionToolbarAction::NewDiagram) => {
                log!("selection toolbar: New Diagram (mock no-op)");
                return;
            }
            _ => {}
        }

        // Doc tab strip: click a tab to activate it, or its close button.
        let tab_action = self
            .ui
            .widget(cx, ids!(doc_tabs))
            .borrow_mut::<crate::doc_tabs::DocTabs>()
            .and_then(|tabs| tabs.tab_action(actions));
        match tab_action {
            Some(crate::doc_tabs::DocTabsAction::Activate(id)) => {
                self.tabs.activate(id);
                self.refresh_doc_tabs(cx);
                self.sync_active_tab(cx);
            }
            Some(crate::doc_tabs::DocTabsAction::Close(id)) => {
                self.tabs.close(id);
                self.refresh_doc_tabs(cx);
                self.sync_active_tab(cx);
            }
            _ => {}
        }
    }
}

impl AppMain for App {
    fn script_mod(vm: &mut ScriptVm) -> ScriptValue {
        crate::makepad_widgets::script_mod(vm);
        crate::theme_atlas::script_mod(vm);
        crate::icons::script_mod(vm);
        crate::draw_hud::script_mod(vm);
        crate::canvas::script_mod(vm);
        crate::tree_panel::script_mod(vm);
        crate::inspector_panel::script_mod(vm);
        crate::doc_tabs::script_mod(vm);
        crate::diagram_switcher::script_mod(vm);
        crate::shortcuts_overlay::script_mod(vm);
        crate::tool_dock::script_mod(vm);
        crate::selection_toolbar::script_mod(vm);
        crate::statusbar::script_mod(vm);
        crate::logo::script_mod(vm);
        crate::waml_button::script_mod(vm);
        crate::start_screen::script_mod(vm);
        self::script_mod(vm)
    }

    fn handle_event(&mut self, cx: &mut Cx, event: &Event) {
        // Tool-dock hotkeys (V/N/C): global, visual-only mode switch. Only
        // live while nothing holds key focus, so they don't fight with the
        // inspector's inline-edit text entry.
        if let Event::KeyDown(ke) = event {
            if cx.key_focus() == Area::Empty {
                let letter = match ke.key_code {
                    KeyCode::KeyV => Some('V'),
                    KeyCode::KeyN => Some('N'),
                    KeyCode::KeyC => Some('C'),
                    _ => None,
                };
                if let Some(tool) = letter.and_then(crate::tool_dock::tool_for_hotkey) {
                    if let Some(mut dock) = self
                        .ui
                        .widget(cx, ids!(tool_dock))
                        .borrow_mut::<crate::tool_dock::ToolDock>()
                    {
                        dock.set_active(cx, tool);
                    }
                    self.sync_statusbar(cx);
                }
                // Shortcuts overlay (U8): `?` opens it, `Escape` closes it --
                // same global-hotkey guard (nothing holding key focus) as
                // the tool-dock modes above.
                match ke.key_code {
                    KeyCode::Slash => self.toggle_shortcuts_overlay(cx),
                    KeyCode::Escape => self.set_shortcuts_overlay(cx, false),
                    _ => {}
                }
            }
        }
        self.match_event(cx, event);
        self.ui.handle_event(cx, event, &mut Scope::empty());

        // The Window widget marks the entire caption bar (minus the window
        // min/max/close buttons) as an OS window-drag region, which swallows
        // pointer events over the doc-tab strip living there -- tab clicks and
        // hover never reach the widget. Re-answer the drag query as `Client`
        // over the tab strip so it behaves as a normal interactive area. This
        // runs after `ui.handle_event`, so this `set` overrides the Window's
        // `Caption` answer (last write wins before the platform reads it).
        if let Event::WindowDragQuery(dq) = event {
            let over_tab = self
                .ui
                .widget(cx, ids!(doc_tabs))
                .borrow::<crate::doc_tabs::DocTabs>()
                .map(|tabs| tabs.hits_any_tab(dq.abs))
                .unwrap_or(false);
            if over_tab {
                dq.response.set(WindowDragQueryResponse::Client);
            }
        }
    }
}
