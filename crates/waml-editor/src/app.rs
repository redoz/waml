use crate::doc_tabs::{OpenTabs, TabKind};
use crate::inspector::Subject;
use crate::load;
use crate::scene::{build_focus_scene, build_scene};
use makepad_widgets::*;
use waml::model::Model;

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.GraphCanvas
    use mod.widgets.ProjectTree
    use mod.widgets.Inspector
    use mod.widgets.DocTabs
    use mod.widgets.ToolDock
    use mod.widgets.SolidView
    use mod.widgets.DesktopButton
    use mod.widgets.DesktopButtonType

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
                    draw_bg.color: #x24242f
                    wordmark := View{
                        width: Fit
                        height: Fill
                        align: Align{x: 0.0, y: 0.5}
                        padding: Inset{left: 12.0, right: 10.0, top: 8.0, bottom: 8.0}
                        Label{
                            text: "WAML"
                            draw_text +: {
                                color: #xf0f0f6
                                text_style: theme.font_bold{font_size: 22}
                            }
                        }
                    }
                    pkg_name_view := View{
                        width: Fill
                        height: Fill
                        align: Center
                        pkg_name := Label{
                            text: ""
                            draw_text +: {
                                color: #xc8c8d4
                                text_style: theme.font_regular{font_size: 13}
                            }
                        }
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
                    Splitter{
                        width: Fill
                        height: Fill
                        axis: SplitterAxis.Horizontal
                        align: SplitterAlign.FromA(280.0)
                        a: View{
                            width: Fill
                            height: Fill
                            project_tree := ProjectTree{
                                width: Fill
                                height: Fill
                            }
                        }
                        b: View{
                            width: Fill
                            height: Fill
                            Splitter{
                                width: Fill
                                height: Fill
                                axis: SplitterAxis.Horizontal
                                align: SplitterAlign.FromB(320.0)
                                a: View{
                                    width: Fill
                                    height: Fill
                                    flow: Down
                                    doc_tabs := DocTabs{
                                        width: Fill
                                        height: 34.0
                                    }
                                    View{
                                        width: Fill
                                        height: Fill
                                        flow: Right
                                        tool_dock := ToolDock{
                                            width: 48.0
                                            height: Fill
                                        }
                                        canvas := GraphCanvas{
                                            width: Fill
                                            height: Fill
                                        }
                                    }
                                }
                                b: View{
                                    width: Fill
                                    height: Fill
                                    inspector := Inspector{
                                        width: Fill
                                        height: Fill
                                    }
                                }
                            }
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
                let built =
                    self.model.diagrams.iter().find(|d| d.key == active.key).map(|d| build_scene(&self.model, d));
                if let Some((scene, diags)) = built {
                    for d in &diags {
                        log!("diagnostic: {d:?}");
                    }
                    if let Some(mut canvas) =
                        self.ui.widget(cx, ids!(canvas)).borrow_mut::<crate::canvas::GraphCanvas>()
                    {
                        canvas.set_scene(cx, scene);
                    }
                }
                if let Some(mut inspector) =
                    self.ui.widget(cx, ids!(inspector)).borrow_mut::<crate::inspector_panel::Inspector>()
                {
                    inspector.set_subject(cx, &self.model, Subject::None);
                }
            }
            TabKind::Classifier => {
                let scene = build_focus_scene(&self.model, &active.key);
                if let Some(mut canvas) =
                    self.ui.widget(cx, ids!(canvas)).borrow_mut::<crate::canvas::GraphCanvas>()
                {
                    canvas.set_focus(cx, scene);
                }
                if let Some(mut inspector) =
                    self.ui.widget(cx, ids!(inspector)).borrow_mut::<crate::inspector_panel::Inspector>()
                {
                    inspector.set_subject(cx, &self.model, Subject::Classifier(active.key.clone()));
                }
            }
        }
    }

    fn refresh_doc_tabs(&mut self, cx: &mut Cx) {
        if let Some(mut doc_tabs) = self.ui.widget(cx, ids!(doc_tabs)).borrow_mut::<crate::doc_tabs::DocTabs>() {
            doc_tabs.set_tabs(cx, &self.tabs);
        }
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
        let model = match load::load_model(&args.dir) {
            Ok(m) => m,
            Err(e) => {
                log!("failed to load OKF dir {:?}: {e}", args.dir);
                return;
            }
        };
        self.model = model;

        let root_name = if self.model.path.is_empty() {
            "bundle"
        } else {
            self.model.path.as_str()
        };
        self.ui
            .label(cx, ids!(pkg_name))
            .set_text(cx, root_name);

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

        let Some(diagram) = crate::cli::select_diagram(&self.model, args.diagram.as_deref()) else {
            log!("no diagrams in {:?}", args.dir);
            return;
        };
        let (scene, diags) = build_scene(&self.model, diagram);
        for d in &diags {
            log!("diagnostic: {d:?}");
        }
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
                let title = node.concept.title.clone().unwrap_or_else(|| node.key.clone());
                self.tabs.open_preview(key, title);
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
            let Some(diagram) = self.model.diagrams.iter().find(|d| d.key == key) else {
                log!("SelectDiagram: no diagram with key {key:?}");
                return;
            };
            if let Some(base) = self.tabs.tabs.first_mut() {
                base.key = diagram.key.clone();
                base.title = diagram.title.clone();
            }
            let base_id = self.tabs.tabs.first().map(|t| t.id).unwrap_or_default();
            self.tabs.activate(base_id);
            self.refresh_doc_tabs(cx);
            self.sync_active_tab(cx);
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

        // Tool dock: mode clicks (Select/Add/Connect) update their own
        // highlight already; one-shot action buttons are mock no-ops here
        // (Shortcuts gets a real overlay in a later unit).
        let dock_action = self
            .ui
            .widget(cx, ids!(tool_dock))
            .borrow_mut::<crate::tool_dock::ToolDock>()
            .and_then(|dock| dock.dock_action(actions));
        if let Some(action) = dock_action {
            log!("tool dock: {action:?}");
            return;
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
        crate::canvas::script_mod(vm);
        crate::tree_panel::script_mod(vm);
        crate::inspector_panel::script_mod(vm);
        crate::doc_tabs::script_mod(vm);
        crate::tool_dock::script_mod(vm);
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
                    if let Some(mut dock) =
                        self.ui.widget(cx, ids!(tool_dock)).borrow_mut::<crate::tool_dock::ToolDock>()
                    {
                        dock.set_active(cx, tool);
                    }
                }
            }
        }
        self.match_event(cx, event);
        self.ui.handle_event(cx, event, &mut Scope::empty());
    }
}
