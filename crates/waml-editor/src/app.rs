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
                                    canvas := GraphCanvas{
                                        width: Fill
                                        height: Fill
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
    }

    fn handle_actions(&mut self, cx: &mut Cx, actions: &Actions) {
        // Classifier focus: single-click a class row -> render that one node in
        // the canvas and point the inspector at it.
        let focused = self
            .ui
            .widget(cx, ids!(project_tree))
            .borrow_mut::<crate::tree_panel::ProjectTree>()
            .and_then(|panel| panel.focused_classifier(actions));
        if let Some(key) = focused {
            if self.model.nodes.iter().any(|n| n.key == key) {
                let scene = build_focus_scene(&self.model, &key);
                if let Some(mut canvas) = self
                    .ui
                    .widget(cx, ids!(canvas))
                    .borrow_mut::<crate::canvas::GraphCanvas>()
                {
                    canvas.set_scene(cx, scene);
                }
                if let Some(mut inspector) = self
                    .ui
                    .widget(cx, ids!(inspector))
                    .borrow_mut::<crate::inspector_panel::Inspector>()
                {
                    inspector.set_subject(cx, &self.model, Subject::Classifier(key));
                }
            }
            return;
        }

        let selected = self
            .ui
            .widget(cx, ids!(project_tree))
            .borrow_mut::<crate::tree_panel::ProjectTree>()
            .and_then(|panel| panel.selected_diagram(actions));
        let Some(key) = selected else {
            return;
        };

        // Rebuild the scene for the clicked diagram. `built` is owned, so the
        // borrow of `self.model` ends before the `self.ui` borrows below.
        let built = self
            .model
            .diagrams
            .iter()
            .find(|d| d.key == key)
            .map(|d| build_scene(&self.model, d));
        let Some((scene, diags)) = built else {
            log!("SelectDiagram: no diagram with key {key:?}");
            return;
        };
        for d in &diags {
            log!("diagnostic: {d:?}");
        }
        if let Some(mut canvas) = self
            .ui
            .widget(cx, ids!(canvas))
            .borrow_mut::<crate::canvas::GraphCanvas>()
        {
            canvas.set_scene(cx, scene);
        }
    }
}

impl AppMain for App {
    fn script_mod(vm: &mut ScriptVm) -> ScriptValue {
        crate::makepad_widgets::script_mod(vm);
        crate::canvas::script_mod(vm);
        crate::tree_panel::script_mod(vm);
        crate::inspector_panel::script_mod(vm);
        self::script_mod(vm)
    }

    fn handle_event(&mut self, cx: &mut Cx, event: &Event) {
        self.match_event(cx, event);
        self.ui.handle_event(cx, event, &mut Scope::empty());
    }
}
