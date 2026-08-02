//! `BehaviorSurface` render passes: `Empty` draws the Atlas background and a
//! centered message; `Flow` draws the solved flow scene (Task 7);
//! `Interaction` draws the solved sequence scene (Task 8).

mod flow;
mod interaction;

use super::hit::BehaviorTarget;
use super::scene::BehaviorScene;
use crate::canvas::linework::{BehaviorLineworkMetrics, DEFAULT_LINEWORK_MODE};
use crate::canvas::viewport::ViewportSnapshot;
use makepad_widgets::*;

pub(super) use flow::FlowDrawResources;
pub(super) use interaction::InteractionDrawResources;

/// Base body/heading text size at zoom 1, in POINTS -- the unit
/// `TextStyle::font_size` is in, which makepad itself rasterizes at
/// `pts * 96/72` lpx. The two solvers measure the same 13pt in LPX
/// (`FlowConfig`/`InteractionConfig::font_size` is `13.0 * PT_TO_LPX`), so this
/// must stay in points: converting here as well applied `96/72` twice and drew
/// every glyph a third wider than the box the solver sized around it.
const BASE_TEXT_PT: f32 = 13.0;

/// Message signature labels, in POINTS at zoom 1. Must track
/// `InteractionConfig::message_font_size`, which the solver measures the label
/// rects with.
pub(super) const MESSAGE_TEXT_PT: f32 = 10.0;

/// Arrow-head extent, in lpx at zoom 1 -- one value for flow routes and
/// interaction messages alike.
pub(super) const ARROW_HEAD: f64 = 9.0;

/// Point a text pen at `zoom`: pick the nearest raster rung for the zoomed
/// target size and carry the remainder in `font_scale`, exactly as every class
/// renderer does. Without this the glyph geometry stays at the DSL size while
/// the boxes scale, so text detaches from its box at any zoom != 1.
pub(super) fn apply_text_zoom(text: &mut DrawText, zoom: f64) {
    apply_text_zoom_pt(text, zoom, BASE_TEXT_PT);
}

/// [`apply_text_zoom`] at an explicit base size in points.
pub(super) fn apply_text_zoom_pt(text: &mut DrawText, zoom: f64, base_pt: f32) {
    let target = (base_pt * zoom as f32).max(4.0);
    let font_size = crate::canvas::primitives::font_raster_size(target);
    text.text_style.font_size = font_size;
    text.font_scale = target / font_size;
}

/// The theme colours the behavior renderers stroke with. Every value is an
/// `atlas` role read off `BehaviorSurface`'s live fields, never a literal, so
/// the behavior canvas follows light/dark exactly as the rest of the chrome
/// does.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct BehaviorPalette {
    /// Resting stroke for a route, lifeline stem, or fragment frame.
    pub(super) line: Vec4,
    /// Transient hover call-out.
    pub(super) hovered: Vec4,
    /// Persistent selection call-out.
    pub(super) selected: Vec4,
}

/// How strongly one element is called out. A click's persistent selection
/// reads stronger than a transient hover (spec §5.2).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum Emphasis {
    #[default]
    None,
    Hovered,
    Selected,
}

impl Emphasis {
    /// `selected` wins over `hovered` (the pointer is usually over whatever was
    /// just clicked).
    pub(super) fn of(hovered: bool, selected: bool) -> Emphasis {
        match (selected, hovered) {
            (true, _) => Emphasis::Selected,
            (false, true) => Emphasis::Hovered,
            (false, false) => Emphasis::None,
        }
    }

    /// Extra wash alpha over the resting fill.
    pub(super) fn wash_boost(self) -> f32 {
        match self {
            Emphasis::None => 0.0,
            Emphasis::Hovered => 0.14,
            Emphasis::Selected => 0.28,
        }
    }

    /// Stroke colour for a line/route/stem at this emphasis.
    pub(super) fn stroke(self, resting: Vec4, palette: BehaviorPalette) -> Vec4 {
        match self {
            Emphasis::None => resting,
            Emphasis::Hovered => palette.hovered,
            Emphasis::Selected => palette.selected,
        }
    }

    /// Stroke thickness at this emphasis -- deliberately the resting weight at
    /// every level. Hover and selection are called out with colour and wash
    /// only (redoz@): a CAD drawing keeps one pen weight per line role, and
    /// fattening a lifeline or route under the pointer reads as a different
    /// kind of line rather than the same line highlighted.
    pub(super) fn thickness(self, resting: f64) -> f64 {
        resting
    }
}

pub(super) struct BehaviorDrawResources<'a> {
    pub(super) bg: &'a mut DrawColor,
    pub(super) text: &'a mut DrawText,
    pub(super) node_box: &'a mut DrawColor,
    pub(super) diamond: &'a mut DrawColor,
    pub(super) circle: &'a mut DrawColor,
    pub(super) triangle: &'a mut DrawColor,
    pub(super) fill: &'a mut DrawColor,
    pub(super) text_heading: &'a mut DrawText,
    pub(super) open_head: &'a mut DrawColor,
    pub(super) x_mark: &'a mut DrawColor,
    pub(super) frame_border: &'a mut DrawColor,
    pub(super) pentagon: &'a mut DrawColor,
    pub(super) head: &'a mut DrawColor,
    pub(super) accent: Vec4,
    pub(super) palette: BehaviorPalette,
}

pub(super) fn draw(
    cx: &mut Cx2d,
    viewport: ViewportSnapshot,
    scene: &BehaviorScene,
    hovered: Option<&BehaviorTarget>,
    selected: Option<&BehaviorTarget>,
    draws: &mut BehaviorDrawResources<'_>,
) {
    let rect = viewport.view_rect;
    draws.bg.draw_abs(cx, rect);
    // One policy for every stroke in this pass: CAD holds linework in screen
    // space at any zoom, `Scaled` grows it with the camera.
    let linework = BehaviorLineworkMetrics::for_zoom(DEFAULT_LINEWORK_MODE, viewport.camera.zoom);
    if !matches!(scene, BehaviorScene::Empty { .. }) {
        // The empty-state message is chrome, not scene content: it stays at its
        // DSL size. Everything else is world-space and scales with the camera.
        apply_text_zoom(draws.text, viewport.camera.zoom);
        apply_text_zoom(draws.text_heading, viewport.camera.zoom);
    }
    match scene {
        BehaviorScene::Empty { message } => draw_message(cx, viewport, message, &mut *draws.text),
        BehaviorScene::Flow {
            nodes,
            edges,
            off_page,
            groups,
        } => {
            let mut flow_draws = FlowDrawResources {
                node_box: &mut *draws.node_box,
                diamond: &mut *draws.diamond,
                circle: &mut *draws.circle,
                triangle: &mut *draws.triangle,
                fill: &mut *draws.fill,
                text_heading: &mut *draws.text_heading,
                text_body: &mut *draws.text,
                palette: draws.palette,
                linework,
            };
            flow::draw(
                cx,
                viewport,
                draws.accent,
                nodes,
                edges,
                off_page,
                groups,
                hovered,
                selected,
                &mut flow_draws,
            )
        }
        BehaviorScene::Interaction {
            lifelines,
            activations,
            messages,
            fragments,
        } => {
            let mut interaction_draws = InteractionDrawResources {
                fill: &mut *draws.fill,
                triangle: &mut *draws.triangle,
                open_head: &mut *draws.open_head,
                x_mark: &mut *draws.x_mark,
                frame_border: &mut *draws.frame_border,
                pentagon: &mut *draws.pentagon,
                head: &mut *draws.head,
                text: &mut *draws.text,
                text_heading: &mut *draws.text_heading,
                palette: draws.palette,
                linework,
            };
            interaction::draw(
                cx,
                viewport,
                draws.accent,
                lifelines,
                activations,
                messages,
                fragments,
                hovered,
                selected,
                &mut interaction_draws,
            )
        }
    }
}

fn draw_message(cx: &mut Cx2d, viewport: ViewportSnapshot, message: &str, text: &mut DrawText) {
    let rect = viewport.view_rect;
    if message.is_empty() {
        return;
    }
    let size = text
        .layout(cx, 0.0, 0.0, None, false, Align::default(), message)
        .size_in_lpxs;
    let pos = dvec2(
        rect.pos.x + (rect.size.x - size.width as f64) * 0.5,
        rect.pos.y + (rect.size.y - size.height as f64) * 0.5,
    );
    text.draw_abs(cx, pos, message);
}
