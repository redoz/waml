//! Canvas view bar (spec 2026-07-24-canvas-view-bar-design §1): a bottom-centre
//! icon strip owning every *view-level* control — the camera one-shots (zoom
//! in/out, fit to size, fit to selection) and the independent view toggles
//! (hidden group borders, constraint veils).
//!
//! Deliberately a separate widget from `ToolDock`, not a section of it: the
//! dock's "lit" means *one exclusive active mode* (`Select`/`Add`/`Connect`),
//! while these are N independent toggles plus one-shot actions. One widget
//! would make "lit" mean two different things.
//!
//! Built on the `ToolDock` pattern: a `#[deref] View` laying out `IconButton`
//! children in a `flow: Right` strip; `draw_walk` syncs each child's glyph +
//! lit state from the owned toggle bools, and `handle_event` reads each child's
//! `clicked` to emit a `ViewBarAction`. The strip's own `draw_bg` paints the
//! Atlas HUD frame, matching `ToolDock`.

use makepad_widgets::*;

use crate::icon_button::IconButtonWidgetRefExt;
use crate::icons::Icon;

script_mod! {
    use mod.prelude.widgets_internal.*
    use mod.atlas
    use mod.widgets.*

    mod.widgets.ViewBarBase = #(ViewBar::register_widget(vm))

    mod.widgets.ViewBar = set_type_default() do mod.widgets.ViewBarBase{
        // Fit so a seventh button later needs no arithmetic.
        width: Fit
        height: 36.0
        flow: Right
        align: Align{x: 0.5, y: 0.5}
        padding: Inset{left: 4.0, right: 4.0, top: 2.0, bottom: 2.0}
        spacing: 2.0
        show_bg: true
        // The strip carries the Atlas HUD frame -- the AccentFrame material
        // inlined onto the View's `draw_bg` (keep in sync with `frame.rs` /
        // `tool_dock.rs`): a `field_bg` fill ringed by the source-bright accent
        // stroke fading along a 150deg diagonal.
        draw_bg +: {
            color: atlas.field_bg
            border_hi: uniform(atlas.frame_hi)
            border_lo: uniform(atlas.frame_lo)
            pixel: fn() {
                let inset = 1.5
                let sdf = Sdf2d.viewport(self.pos * self.rect_size)
                sdf.rect(inset, inset, self.rect_size.x - inset * 2.0, self.rect_size.y - inset * 2.0)
                sdf.fill_keep(self.color)
                let dir = vec2(0.5, 0.8660254)
                let span = 1.3660254
                let t = clamp((self.pos.x * dir.x + self.pos.y * dir.y) / span, 0.0, 1.0)
                sdf.stroke(mix(self.border_hi, self.border_lo, t), inset)
                return sdf.result
            }
        }

        // Camera one-shots (never lit), then a hairline divider, then the
        // independent view toggles (lit while on).
        zoom_in_btn := IconButton {}
        zoom_out_btn := IconButton {}
        fit_size_btn := IconButton {}
        fit_selection_btn := IconButton {}
        divider := View{
            width: 1.0
            height: Fill
            show_bg: true
            margin: Inset{left: 5.0, right: 5.0, top: 6.0, bottom: 6.0}
            draw_bg +: { color: atlas.frame_lo }
        }
        hidden_borders_btn := IconButton {}
        constraints_btn := IconButton {}
    }
}

/// A view-bar entry. The first four are one-shot camera *actions*; the last two
/// are independent *toggles* (lit while on).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ViewOption {
    ZoomIn,
    ZoomOut,
    FitToSize,
    FitToSelection,
    ShowHiddenBorders,
    ShowConstraints,
}

impl ViewOption {
    /// Declaration order == left-to-right layout order.
    pub const ALL: [ViewOption; 6] = [
        ViewOption::ZoomIn,
        ViewOption::ZoomOut,
        ViewOption::FitToSize,
        ViewOption::FitToSelection,
        ViewOption::ShowHiddenBorders,
        ViewOption::ShowConstraints,
    ];

    /// Independent on/off state (lit while on) vs. a one-shot action.
    pub fn is_toggle(self) -> bool {
        matches!(
            self,
            ViewOption::ShowHiddenBorders | ViewOption::ShowConstraints
        )
    }

    /// Human-readable name. No consumer yet -- the bar has no tooltips and the
    /// statusbar doesn't report view state; kept because it is the natural home
    /// for that copy and the tests pin it.
    #[allow(dead_code)]
    pub fn label(self) -> &'static str {
        match self {
            ViewOption::ZoomIn => "Zoom In",
            ViewOption::ZoomOut => "Zoom Out",
            ViewOption::FitToSize => "Fit to Size",
            ViewOption::FitToSelection => "Fit to Selection",
            ViewOption::ShowHiddenBorders => "Show Hidden Borders",
            ViewOption::ShowConstraints => "Show Constraints",
        }
    }
}

/// The bar's independent toggle state. A plain struct rather than two widget
/// fields so the option->state map is unit-testable without constructing a
/// `View` (makepad widgets aren't constructible outside a running `Cx`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ViewToggles {
    /// Constraint veils on/off.
    show_constraints: bool,
    /// X-ray for groups that opt out of chrome.
    show_hidden_borders: bool,
}

impl Default for ViewToggles {
    /// Constraints default ON so the bar preserves today's behaviour
    /// (`ConstraintVisibility::default()` is `Selected`); hidden borders OFF.
    fn default() -> ViewToggles {
        ViewToggles {
            show_constraints: true,
            show_hidden_borders: false,
        }
    }
}

impl ViewToggles {
    /// Current state of a toggle. `false` for a one-shot option (never lit).
    ///
    /// The one-shots are listed out rather than caught by a `_` arm so that a
    /// future `ViewOption` (Plans C/D extend this enum) whose `is_toggle()` is
    /// `true` breaks the build here instead of silently reading back `false`.
    fn get(self, opt: ViewOption) -> bool {
        match opt {
            ViewOption::ShowConstraints => self.show_constraints,
            ViewOption::ShowHiddenBorders => self.show_hidden_borders,
            ViewOption::ZoomIn
            | ViewOption::ZoomOut
            | ViewOption::FitToSize
            | ViewOption::FitToSelection => false,
        }
    }

    /// Store a toggle's new state. A one-shot option is ignored -- again spelled
    /// out per variant so a new toggle can't silently drop its writes.
    fn set(&mut self, opt: ViewOption, on: bool) {
        match opt {
            ViewOption::ShowConstraints => self.show_constraints = on,
            ViewOption::ShowHiddenBorders => self.show_hidden_borders = on,
            ViewOption::ZoomIn
            | ViewOption::ZoomOut
            | ViewOption::FitToSize
            | ViewOption::FitToSelection => {}
        }
    }
}

#[derive(Clone, Debug, Default)]
pub enum ViewBarAction {
    #[default]
    None,
    /// A camera one-shot fired. The `ViewOption` payload is kept for the
    /// `log!` in `class_diagram_view.rs` (Debug-only) while these buttons stay
    /// `log!` no-ops -- Plan D wires the camera actions.
    Triggered(#[allow(dead_code)] ViewOption),
    /// A toggle flipped; carries its new state.
    Toggled(ViewOption, bool),
}

#[derive(Script, ScriptHook, Widget)]
pub struct ViewBar {
    /// The strip: a `flow: Right` `View` whose `draw_bg` paints the HUD frame
    /// and which lays out the six `IconButton` children plus the divider.
    #[deref]
    view: View,

    /// The two independent view toggles (constraint veils, hidden borders).
    #[rust]
    toggles: ViewToggles,
}

impl Widget for ViewBar {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        // Drive the children so their `clicked`/hover actions are emitted.
        self.view.handle_event(cx, event, scope);

        let uid = self.widget_uid();
        if let Event::Actions(actions) = event {
            for opt in ViewOption::ALL {
                if self.button(cx, opt).as_icon_button().clicked(actions) {
                    if opt.is_toggle() {
                        let on = !self.toggles.get(opt);
                        self.toggles.set(opt, on);
                        self.view.redraw(cx);
                        cx.widget_action(uid, ViewBarAction::Toggled(opt, on));
                    } else {
                        cx.widget_action(uid, ViewBarAction::Triggered(opt));
                    }
                    break;
                }
            }
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        // Sync each child's glyph + lit state before the View lays them out:
        // only a toggle that is ON is lit; one-shot buttons never are.
        for opt in ViewOption::ALL {
            let lit = opt.is_toggle() && self.toggles.get(opt);
            let btn = self.button(cx, opt).as_icon_button();
            btn.set_icon(cx, Self::icon_for(opt));
            btn.set_active(cx, lit);
        }

        while self.view.draw_walk(cx, scope, walk).step().is_some() {}

        DrawStep::done()
    }
}

impl ViewBar {
    /// The child `IconButton` for an option. Central option->widget map, shared
    /// by the draw-time sync and the event-time click read.
    fn button(&mut self, cx: &mut Cx, opt: ViewOption) -> WidgetRef {
        match opt {
            ViewOption::ZoomIn => self.view.widget(cx, ids!(zoom_in_btn)),
            ViewOption::ZoomOut => self.view.widget(cx, ids!(zoom_out_btn)),
            ViewOption::FitToSize => self.view.widget(cx, ids!(fit_size_btn)),
            ViewOption::FitToSelection => self.view.widget(cx, ids!(fit_selection_btn)),
            ViewOption::ShowHiddenBorders => self.view.widget(cx, ids!(hidden_borders_btn)),
            ViewOption::ShowConstraints => self.view.widget(cx, ids!(constraints_btn)),
        }
    }

    /// The catalog glyph for an option. Pure meaning->glyph map; the child
    /// `IconButton` fetches the shader and tints it per-draw.
    fn icon_for(opt: ViewOption) -> Icon {
        match opt {
            ViewOption::ZoomIn => Icon::ZoomIn,
            ViewOption::ZoomOut => Icon::ZoomOut,
            ViewOption::FitToSize => Icon::Maximize,
            ViewOption::FitToSelection => Icon::ScanSearch,
            ViewOption::ShowHiddenBorders => Icon::SquareDashed,
            ViewOption::ShowConstraints => Icon::Ruler,
        }
    }

    /// Push canvas-side state back into the bar's `ShowConstraints` toggle.
    ///
    /// The canvas is the source of truth for the veil mode; the bar's bool is a
    /// mirror of it. Without this the widget's own click handler is the only
    /// writer, so any drift (a tab activation, a future keyboard/programmatic
    /// toggle) would leave the lit state lying until a restart. Repaints only
    /// on a real change so the per-`sync` call is free.
    pub fn set_show_constraints(&mut self, cx: &mut Cx, on: bool) {
        if self.toggles.get(ViewOption::ShowConstraints) != on {
            self.toggles.set(ViewOption::ShowConstraints, on);
            self.view.redraw(cx);
        }
    }

    /// Convenience reader for the active `DocView`, mirroring
    /// `ToolDock::dock_action`.
    pub fn view_bar_action(&self, actions: &Actions) -> Option<ViewBarAction> {
        let item = actions.find_widget_action(self.widget_uid())?;
        match item.cast() {
            ViewBarAction::None => None,
            action => Some(action),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::icons::Icon;

    #[test]
    fn all_lists_six_options_in_layout_order() {
        assert_eq!(ViewOption::ALL.len(), 6);
        assert_eq!(ViewOption::ALL[0], ViewOption::ZoomIn);
        assert_eq!(ViewOption::ALL[1], ViewOption::ZoomOut);
        assert_eq!(ViewOption::ALL[2], ViewOption::FitToSize);
        assert_eq!(ViewOption::ALL[3], ViewOption::FitToSelection);
        assert_eq!(ViewOption::ALL[4], ViewOption::ShowHiddenBorders);
        assert_eq!(ViewOption::ALL[5], ViewOption::ShowConstraints);
    }

    #[test]
    fn only_the_last_two_options_are_toggles() {
        for (i, opt) in ViewOption::ALL.iter().enumerate() {
            assert_eq!(opt.is_toggle(), i >= 4, "{opt:?} toggle-ness mismatch");
        }
    }

    #[test]
    fn every_option_maps_to_a_catalog_icon() {
        assert_eq!(ViewBar::icon_for(ViewOption::ZoomIn), Icon::ZoomIn);
        assert_eq!(ViewBar::icon_for(ViewOption::ZoomOut), Icon::ZoomOut);
        assert_eq!(ViewBar::icon_for(ViewOption::FitToSize), Icon::Maximize);
        assert_eq!(
            ViewBar::icon_for(ViewOption::FitToSelection),
            Icon::ScanSearch
        );
        assert_eq!(
            ViewBar::icon_for(ViewOption::ShowHiddenBorders),
            Icon::SquareDashed
        );
        assert_eq!(ViewBar::icon_for(ViewOption::ShowConstraints), Icon::Ruler);
    }

    #[test]
    fn toggles_default_to_constraints_on_borders_off() {
        // `ConstraintVisibility::default()` is `Selected`, so the veil toggle
        // must start lit or the bar and the canvas disagree on first paint.
        let t = ViewToggles::default();
        assert!(t.get(ViewOption::ShowConstraints));
        assert!(!t.get(ViewOption::ShowHiddenBorders));
    }

    #[test]
    fn set_flips_only_the_named_toggle() {
        let mut t = ViewToggles::default();
        t.set(ViewOption::ShowConstraints, false);
        assert!(!t.get(ViewOption::ShowConstraints));
        assert!(!t.get(ViewOption::ShowHiddenBorders));
        t.set(ViewOption::ShowHiddenBorders, true);
        assert!(!t.get(ViewOption::ShowConstraints));
        assert!(t.get(ViewOption::ShowHiddenBorders));
    }

    #[test]
    fn one_shot_options_are_never_on() {
        let mut t = ViewToggles::default();
        for opt in ViewOption::ALL.iter().filter(|o| !o.is_toggle()) {
            t.set(*opt, true);
            assert!(!t.get(*opt), "{opt:?} is a one-shot, never lit");
        }
        // ...and setting one never disturbs the real toggles.
        assert_eq!(t, ViewToggles::default());
    }

    #[test]
    fn every_option_has_a_nonempty_label() {
        for opt in ViewOption::ALL {
            assert!(!opt.label().is_empty(), "empty label for {opt:?}");
        }
        assert_eq!(ViewOption::ShowConstraints.label(), "Show Constraints");
    }
}
