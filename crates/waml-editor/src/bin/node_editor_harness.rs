//! Harness for `NodeDesignEditor`: mounts the widget in a bare window on the
//! Atlas light ground so the frosted HUD pane + live preview + controls can be
//! judged in context. The widget is compiled into the crate but never mounted in
//! the live app (`app.rs`); this bin is the only way to view it.
//!
//! Run: `cargo run -p waml-editor --bin node_editor_harness`
//! No hot-reload in a bare `cargo run` -- edit `node_design_editor.rs`, rebuild,
//! relaunch. Shader/DSL errors surface at GPU runtime in stdout as `[E] ...`.

use makepad_widgets::*;

// Pulled in by path (the editor crate has no lib target). `frame` supplies the
// shared `mod.draw.AccentFrame` the widget's pane/card/inset surfaces reuse.
#[path = "../theme_atlas.rs"]
mod theme_atlas;
#[path = "../frame.rs"]
mod frame;
// The reusable node-render path the preview draws through. All makepad-free; the
// widget builds a `scene::SceneNode` and renders it via `card::measure`. This bin
// exercises only a slice of each module (the real app uses the rest), so silence
// dead-code here rather than in the shared source.
#[allow(dead_code)]
#[path = "../load.rs"]
mod load;
#[allow(dead_code)]
#[path = "../node_style.rs"]
mod node_style;
#[allow(dead_code)]
#[path = "../sizing.rs"]
mod sizing;
#[allow(dead_code)]
#[path = "../inspector.rs"]
mod inspector;
#[allow(dead_code)]
#[path = "../card/mod.rs"]
mod card;
#[allow(dead_code)]
#[path = "../scene.rs"]
mod scene;
#[path = "../node_design_editor.rs"]
mod node_design_editor;

app_main!(App);

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*
    use mod.draw
    use mod.atlas
    use mod.text.*
    use mod.widgets.NodeDesignEditor

    startup() do #(App::script_component(vm)){
        ui: Root{
            main_window := Window{
                // Atlas light ground (the mock's stage). A flat clear is close
                // enough to judge the pane material against.
                pass.clear_color: vec4(0.933, 0.949, 0.968, 1.0)
                window.inner_size: vec2(980, 680)
                window.title: "WAML node design editor"
                body +: {
                    editor := NodeDesignEditor{
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
    // First-frame kick: the SDF DrawColor bg doesn't paint until its area is
    // invalidated, so force one redraw once the UI is up.
    #[rust]
    kick: NextFrame,
}

impl MatchEvent for App {
    fn handle_startup(&mut self, cx: &mut Cx) {
        self.kick = cx.new_next_frame();
    }
}

impl AppMain for App {
    fn script_mod(vm: &mut ScriptVm) -> ScriptValue {
        makepad_widgets::script_mod(vm);
        crate::theme_atlas::script_mod(vm);
        crate::frame::script_mod(vm);
        crate::node_design_editor::script_mod(vm);
        self::script_mod(vm)
    }

    fn handle_event(&mut self, cx: &mut Cx, event: &Event) {
        if self.kick.is_event(event).is_some() {
            self.ui.redraw(cx);
        }
        self.match_event(cx, event);
        self.ui.handle_event(cx, event, &mut Scope::empty());
    }
}
