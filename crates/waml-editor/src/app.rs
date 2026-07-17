use crate::load;
use crate::scene::build_scene;
use makepad_widgets::*;
use waml::model::Model;

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.GraphCanvas
    use mod.widgets.ProjectTree

    startup() do #(App::script_component(vm)){
        ui: Root{
            main_window := Window{
                window.inner_size: vec2(1280, 840)
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
                            canvas := GraphCanvas{
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

        let Some(diagram) = crate::cli::select_diagram(&self.model, args.diagram.as_deref())
        else {
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
        self::script_mod(vm)
    }

    fn handle_event(&mut self, cx: &mut Cx, event: &Event) {
        self.match_event(cx, event);
        self.ui.handle_event(cx, event, &mut Scope::empty());
    }
}
