//! Harness for `NodeDesignEditor`: mounts the widget in a bare window on the
//! Atlas light ground so the frosted HUD pane + live preview + controls can be
//! judged in context. The widget is compiled into the crate but never mounted in
//! the live app (`app.rs`); this bin is the only way to view it.
//!
//! Run: `cargo run -p waml-editor --bin node_editor_harness`
//! No hot-reload in a bare `cargo run` -- edit `node_design_editor.rs`, rebuild,
//! relaunch. Shader/DSL errors surface at GPU runtime in stdout as `[E] ...`.

use makepad_widgets::*;
// Pulled in from the lib target. `frame` supplies the shared
// `mod.draw.AccentFrame` the widget's pane/card/inset surfaces reuse.
// `node_design_editor` itself reaches the reusable node-render path (`card`,
// `diagram_display`, `edge_labels`, `inspector`, `load`, `node_style`,
// `scene`, `sizing`) through the lib, so this bin needs only what it names
// directly.
use waml_editor::{frame, node_design_editor, theme_atlas};

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
        theme_atlas::script_mod(vm);
        frame::script_mod(vm);
        node_design_editor::script_mod(vm);
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
