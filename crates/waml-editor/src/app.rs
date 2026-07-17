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
        // Directory + optional diagram title come from argv (wired fully in Task 8).
        let dir = std::env::args().nth(1).unwrap_or_else(|| ".".to_string());
        let model = match load::load_model(std::path::Path::new(&dir)) {
            Ok(m) => m,
            Err(e) => {
                log!("failed to load OKF dir {dir:?}: {e}");
                return;
            }
        };
        let Some(diagram) = model.diagrams.first() else {
            log!("no diagrams in {dir:?}");
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
