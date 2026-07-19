//! Side-by-side logo harness: `waml.svg` rendered via `DrawSvg` (left,
//! reference, native SVG colors) next to the `LogoMark` SDF (right). Lets us
//! iterate on the shader and eyeball AA / geometry fidelity, and doubles as a
//! comparison rig for any future SVG icon we want to port to an SDF.
//!
//! Run: `cargo run -p waml-editor --bin logo_harness`
//! The shader recompiles at runtime -- edit `logo.rs`'s `k1..k6` / geometry
//! and relaunch to see changes. Shader errors surface in stdout as `[E] ...`.

use makepad_widgets::*;

// `logo.rs` is self-contained (only depends on `makepad_widgets`), so pull it
// in by path rather than routing through the (lib-less) editor crate.
#[path = "../logo.rs"]
mod logo;

app_main!(App);

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    startup() do #(App::script_component(vm)){
        ui: Root{
            main_window := Window{
                // Light bg so the now-dark stroke ramp (0.15..0.40) reads.
                pass.clear_color: vec4(0.90, 0.90, 0.90, 1.0)
                window.inner_size: vec2(1100, 560)
                window.title: "Logo Harness -- SVG (left) vs SDF (right)"
                body +: {
                    padding: 40
                    flow: Right
                    align: Align{x: 0.5, y: 0.5}
                    spacing: 80

                    // Reference: the raw vector via DrawSvg (Icon's default
                    // color -1,-1,-1,-1 = keep the SVG's own fills).
                    Icon{
                        icon_walk: Walk{ width: 440, height: Fit }
                        draw_icon.svg: crate_resource("self:resources/waml.svg")
                    }
                    // The SDF port. SolidView (its draw_bg is a DrawQuad) so the
                    // LogoMark subclass attaches; box holds the ~1.749 aspect.
                    SolidView{
                        width: 440
                        height: 252
                        draw_bg: mod.draw.LogoMark{}
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
    // First-frame kick: the SDF DrawQuad bg doesn't paint until something
    // invalidates its area (else it stays blank until the first hover), so
    // force one redraw once the UI is up.
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
        logo::script_mod(vm);
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
