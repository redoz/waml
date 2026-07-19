//! Transparent floating host for the logo radial.
//!
//! A thin pass-owning widget -- NOT the heavyweight `Window` widget and NOT a
//! manual `AppMain` draw. It lives as a Root-level sibling of the main `Window`,
//! so on each `Event::Draw` the framework draws its pass *sequentially after* the
//! main window's pass (the multi-window draw idiom -- see makepad's
//! `floating_panel` example), never nested inside another window's pass.
//!
//! On open it creates a transparent `new_popup` OS window (DirectComposition
//! per-pixel alpha), binds its own `DrawPass` to that window, and clears the pass
//! to premultiplied alpha 0 so the desktop shows through the disc's transparent
//! regions. The actual wedge/hub/icon geometry is drawn by an embedded `Radial`
//! (the same widget the in-window node menu uses) in popup-window-local coords;
//! the disc is centred in the popup so the menu blooms around the logo.
//!
//! The popup window's pointer/key events (delivered with popup-local `abs` and
//! the popup `window_id`) are forwarded into the embedded `Radial`; a commit or
//! dismiss closes the OS window.

use crate::radial::{Radial, RadialItem, RadialOutcome, DISC_RADIUS};
use makepad_widgets::*;

script_mod! {
    use mod.prelude.widgets_internal.*
    use mod.widgets.*
    use mod.widgets.Radial

    mod.widgets.RadialPopupBase = #(RadialPopup::register_widget(vm))

    mod.widgets.RadialPopup = set_type_default() do mod.widgets.RadialPopupBase{
        width: Fill
        height: Fill
        radial := Radial{
            width: Fill
            height: Fill
        }
    }
}

/// Disc window edge length (screen px): the full disc diameter plus a small AA
/// margin on each side so the rim never touches the window edge.
const POPUP_MARGIN: f64 = 24.0;
const POPUP_SIZE: f64 = DISC_RADIUS * 2.0 + POPUP_MARGIN * 2.0;

#[derive(Script, ScriptHook, Widget)]
pub struct RadialPopup {
    #[uid]
    uid: WidgetUid,
    #[source]
    source: ScriptObjectRef,
    #[walk]
    walk: Walk,
    #[layout]
    layout: Layout,

    /// The geometry/state engine, drawn into our pass in popup-local coords.
    #[redraw]
    #[live]
    radial: Radial,

    #[rust(DrawPass::new_with_name(vm.cx_mut(), "radial_popup"))]
    pass: DrawPass,
    #[new]
    draw_list: DrawList2d,
    #[rust]
    depth_texture: Option<Texture>,

    #[rust]
    open: bool,
    #[rust]
    window: Option<WindowHandle>,
}

impl Widget for RadialPopup {
    // Event-passive like `Radial`: the parent (`App`) drives this through
    // `handle` below, so a stray tree route can't double-handle a gesture. We
    // only self-handle the framework dismiss here as a safety net.
    fn handle_event(&mut self, _cx: &mut Cx, _event: &Event, _scope: &mut Scope) {}

    fn draw_walk(&mut self, cx: &mut Cx2d, _scope: &mut Scope, _walk: Walk) -> DrawStep {
        if !self.open || self.window.is_none() {
            return DrawStep::done();
        }

        cx.begin_pass(&self.pass, None);
        self.draw_list.begin_always(cx);
        let size = cx.current_pass_size();
        cx.begin_root_turtle(size, Layout::flow_overlay());

        // The embedded radial draws at its stored (popup-local) centre.
        self.radial.draw(cx);

        cx.end_pass_sized_turtle();
        self.draw_list.end(cx);
        cx.end_pass(&self.pass);
        DrawStep::done()
    }
}

#[allow(dead_code)]
impl RadialPopup {
    pub fn is_open(&self) -> bool {
        self.open
    }

    /// Open the transparent popup so the disc centre sits at `center` (main-window
    /// relative logical coords -- e.g. the logo centre). `parent` is the main
    /// window id. The disc window overflows past the main window's top-left edge
    /// onto the desktop when the centre is near the corner.
    pub fn open(
        &mut self,
        cx: &mut Cx,
        parent: WindowId,
        center: DVec2,
        items: Vec<RadialItem>,
        time: f64,
    ) {
        // Popup top-left in parent-relative coords, so the disc centre lands on
        // `center`. May go negative -> overflows onto the desktop.
        let position = dvec2(center.x - POPUP_SIZE * 0.5, center.y - POPUP_SIZE * 0.5);
        let size = dvec2(POPUP_SIZE, POPUP_SIZE);

        let handle = WindowHandle::new_popup(cx, parent, position, size, true);
        handle.set_pass(cx, &self.pass);
        self.pass.set_window_clear_color(cx, vec4(0.0, 0.0, 0.0, 0.0));
        if self.depth_texture.is_none() {
            let depth = Texture::new_with_format(
                cx,
                TextureFormat::DepthD32 {
                    size: TextureSize::Auto,
                    initial: true,
                },
            );
            self.pass
                .set_depth_texture(cx, &depth, DrawPassClearDepth::ClearWith(1.0));
            self.depth_texture = Some(depth);
        }
        self.window = Some(handle);
        self.open = true;

        // Radial blooms around the popup centre (in popup-local coords).
        let local_center = dvec2(POPUP_SIZE * 0.5, POPUP_SIZE * 0.5);
        self.radial.open_popup(cx, local_center, items, time);
    }

    fn popup_window_id(&self) -> Option<WindowId> {
        self.window.as_ref().map(|w| w.window_id())
    }

    /// True if `event` belongs to (or globally affects) the open popup and should
    /// drive the embedded radial. Pointer events are filtered to the popup
    /// window; key/animation/dismiss events are global.
    fn event_targets_popup(&self, event: &Event) -> bool {
        let Some(id) = self.popup_window_id() else {
            return false;
        };
        match event {
            Event::MouseDown(e) => e.window_id == id,
            Event::MouseMove(e) => e.window_id == id,
            Event::MouseUp(e) => e.window_id == id,
            Event::Scroll(e) => e.window_id == id,
            // Key, NextFrame (bloom animation) and dismiss are not window-scoped.
            Event::KeyDown(_) | Event::KeyUp(_) | Event::TextInput(_) => true,
            _ => true,
        }
    }

    /// Drive the open popup with `event`; returns the radial outcome. On a commit
    /// or dismiss the popup OS window is closed. `App` maps the committed id to a
    /// `LogoCommand`.
    pub fn handle(&mut self, cx: &mut Cx, event: &Event) -> RadialOutcome {
        if !self.open {
            return RadialOutcome::None;
        }

        // Framework dismiss (outside click / focus loss) closes and cancels.
        if let Event::PopupDismissed(pd) = event {
            if self.popup_window_id() == Some(pd.window_id) {
                self.close(cx);
                return RadialOutcome::Cancelled;
            }
            return RadialOutcome::None;
        }

        if !self.event_targets_popup(event) {
            return RadialOutcome::None;
        }

        let outcome = self.radial.handle(cx, event);
        match outcome {
            RadialOutcome::None => RadialOutcome::None,
            other => {
                // Committed or Cancelled: tear down the OS window.
                self.close(cx);
                other
            }
        }
    }

    pub fn close(&mut self, cx: &mut Cx) {
        if let Some(mut handle) = self.window.take() {
            handle.close(cx);
        }
        self.open = false;
    }
}
