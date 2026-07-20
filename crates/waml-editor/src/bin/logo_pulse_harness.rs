//! Splash-logo pulse compare harness: the `LogoMark` wordmark drawn once per
//! animation `mode`, stacked, each free-running (`auto: true`) on the splash's
//! light ground so the colour pulse reads in its real context.
//!
//! Rows (see logo.rs `pixel`):
//!   1 accent      2 close-encounters   3 bucket-palette
//!   4 molten      5 neon               6 electric
//!
//! Run: `cargo run -p waml-editor --bin logo_pulse_harness`
//! No hot-reload in a bare `cargo run` -- edit `logo.rs`, rebuild, relaunch.
//! Shader errors surface at GPU runtime in stdout as `[E] ...logo.rs:LINE`.

use makepad_widgets::*;

// Pulled in by path (the editor crate has no lib target).
#[path = "../logo.rs"]
mod logo;
#[path = "../theme_atlas.rs"]
mod theme_atlas;

app_main!(App);

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*
    use mod.draw
    use mod.atlas
    use mod.text.*

    startup() do #(App::script_component(vm)){
        ui: Root{
            main_window := Window{
                // Splash ground is a light radial; a flat light clear is close
                // enough to judge the pulse colours against.
                pass.clear_color: vec4(0.93, 0.94, 0.96, 1.0)
                window.inner_size: vec2(320, 960)
                window.title: "WAML pulse 1-6 (top->bottom)"
                body +: {
                    padding: 28
                    flow: Down
                    spacing: 18.0

                    // Rows top->bottom = modes 1..6:
                    //   1 accent · 2 close-encounters · 3 bucket-palette
                    //   4 molten · 5 neon · 6 electric
                    // `mode` is the widget field (Rust drives the uniform);
                    // clicking a row crossfades it to the next variant.
                    mod.widgets.LogoMark{ width: 240.0, height: 137.0, auto: true, mode: 1.0 }
                    mod.widgets.LogoMark{ width: 240.0, height: 137.0, auto: true, mode: 2.0 }
                    mod.widgets.LogoMark{ width: 240.0, height: 137.0, auto: true, mode: 3.0 }
                    mod.widgets.LogoMark{ width: 240.0, height: 137.0, auto: true, mode: 4.0 }
                    mod.widgets.LogoMark{ width: 240.0, height: 137.0, auto: true, mode: 5.0 }
                    mod.widgets.LogoMark{ width: 240.0, height: 137.0, auto: true, mode: 6.0 }
                }
            }
        }
    }
}

#[derive(Script, ScriptHook)]
pub struct App {
    #[live]
    ui: WidgetRef,
    // First-frame kick: the SDF DrawQuad bg doesn't paint until its area is
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
        crate::logo::script_mod(vm);
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
