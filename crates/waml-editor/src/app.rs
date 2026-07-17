use crate::load;
use crate::scene::build_scene;
use makepad_widgets::*;

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.GraphCanvas

    startup() do #(App::script_component(vm)){
        ui: Root{
            main_window := Window{
                window.inner_size: vec2(1280, 840)
                pass.clear_color: vec4(0.08, 0.09, 0.11, 1.0)
                body +: {
                    canvas := GraphCanvas{
                        width: Fill
                        height: Fill
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
        let Some(diagram) = crate::cli::select_diagram(&model, args.diagram.as_deref()) else {
            log!("no diagrams in {:?}", args.dir);
            return;
        };
        let (scene, diags) = build_scene(&model, diagram);
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
}

impl AppMain for App {
    fn script_mod(vm: &mut ScriptVm) -> ScriptValue {
        crate::makepad_widgets::script_mod(vm);
        crate::canvas::script_mod(vm);
        self::script_mod(vm)
    }

    fn handle_event(&mut self, cx: &mut Cx, event: &Event) {
        self.match_event(cx, event);
        self.ui.handle_event(cx, event, &mut Scope::empty());
    }
}
